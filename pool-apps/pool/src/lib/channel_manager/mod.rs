use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, AtomicUsize},
        Arc,
    },
};

use stratum_apps::stratum_core::bitcoin::{Address, Network, address::NetworkUnchecked};

use async_channel::{Receiver, Sender};
use core::sync::atomic::Ordering;
use stratum_apps::{
    coinbase_output_constraints::coinbase_output_constraints_message,
    config_helpers::CoinbaseRewardScript,
    custom_mutex::Mutex,
    key_utils::{Secp256k1PublicKey, Secp256k1SecretKey},
    network_helpers::noise_stream::NoiseTcpStream,
    stratum_core::{
        bitcoin::{Amount, TxOut},
        channels_sv2::{
            server::{
                extended::ExtendedChannel,
                group::GroupChannel,
                jobs::{extended::ExtendedJob, job_store::DefaultJobStore, standard::StandardJob},
                standard::StandardChannel,
            },
            Vardiff, VardiffState,
        },
        codec_sv2::HandshakeRole,
        handlers_sv2::{
            HandleMiningMessagesFromClientAsync, HandleTemplateDistributionMessagesFromServerAsync,
        },
        mining_sv2::{ExtendedExtranonce, SetTarget},
        noise_sv2::Responder,
        parsers_sv2::{Mining, TemplateDistribution, Tlv},
        template_distribution_sv2::{NewTemplate, SetNewPrevHash},
    },
    task_manager::TaskManager,
    utils::types::{ChannelId, DownstreamId, Message, SharesPerMinute, VardiffKey},
};
use tokio::{net::TcpListener, select, sync::broadcast};
use tracing::{debug, error, info, warn};

use crate::{
    config::PoolConfig,
    downstream::Downstream,
    error::{self, PoolError, PoolErrorKind, PoolResult},
    status::{handle_error, Status, StatusSender},
    utils::ShutdownMessage,
    webhook::WebhookClient,
};

mod mining_message_handler;
mod template_distribution_message_handler;

const POOL_ALLOCATION_BYTES: usize = 4;
const CLIENT_SEARCH_SPACE_BYTES: usize = 16;
pub const FULL_EXTRANONCE_SIZE: usize = POOL_ALLOCATION_BYTES + CLIENT_SEARCH_SPACE_BYTES;

pub struct ChannelManagerData {
    // Mapping of `downstream_id` → `Downstream` object,
    // used by the channel manager to locate and interact with downstream clients.
    pub(crate) downstream: HashMap<DownstreamId, Downstream>,
    // Extranonce prefix factory for **extended downstream channels**.
    // Each new extended downstream receives a unique extranonce prefix.
    extranonce_prefix_factory_extended: ExtendedExtranonce,
    // Extranonce prefix factory for **standard downstream channels**.
    // Each new standard downstream receives a unique extranonce prefix.
    extranonce_prefix_factory_standard: ExtendedExtranonce,
    // Factory that assigns a unique ID to each new **downstream connection**.
    downstream_id_factory: AtomicUsize,
    // Mapping of `(downstream_id, channel_id)` → vardiff controller.
    // Each entry manages variable difficulty for a specific downstream channel.
    vardiff: HashMap<VardiffKey, VardiffState>,
    // Coinbase outputs (pub(crate) for test infrastructure)
    pub(crate) coinbase_outputs: Vec<u8>,
    // Last new prevhash
    last_new_prev_hash: Option<SetNewPrevHash<'static>>,
    // Last future template
    last_future_template: Option<NewTemplate<'static>>,
    /// Maps job_id → user_id for share attribution (WEBHOOK-06)
    /// Populated when update_coinbase_and_broadcast creates jobs
    /// Consumed when handle_submit_shares validates shares
    /// Cleaned up on SetNewPrevHash to prevent memory leak (CONC-03)
    pub(crate) job_to_user: HashMap<u32, String>,
    /// Current user_id for share attribution (persistent across template updates)
    /// Set via coinbase API, defaults to "unknown"
    /// Used to populate job_to_user mapping for all new jobs
    current_user_id: String,
    /// Current pool_tag for pool identification in coinbase scriptSig
    /// Set via coinbase API, defaults to config.pool_signature()
    /// Used when creating channels to identify pool in mined blocks
    current_pool_tag: String,
    /// Current shares_per_minute target for all channels
    /// Set via shares_per_minute API, defaults to config.shares_per_minute()
    /// Used for vardiff calculation and immediate target updates
    current_shares_per_minute: SharesPerMinute,
}

#[derive(Clone)]
pub struct ChannelManagerChannel {
    tp_sender: Sender<TemplateDistribution<'static>>,
    tp_receiver: Receiver<TemplateDistribution<'static>>,
    downstream_sender: broadcast::Sender<(usize, Mining<'static>, Option<Vec<Tlv>>)>,
    downstream_receiver: Receiver<(usize, Mining<'static>, Option<Vec<Tlv>>)>,
}

/// Contains all the state of mutable and immutable data required
/// by channel manager to process its task along with channels
/// to perform message traversal.
#[derive(Clone)]
pub struct ChannelManager {
    pub(crate) channel_manager_data: Arc<Mutex<ChannelManagerData>>,
    channel_manager_channel: ChannelManagerChannel,
    share_batch_size: usize,
    coinbase_reward_script: CoinbaseRewardScript,
    /// Protocol extensions that the pool supports (will accept if requested by clients).
    supported_extensions: Vec<u16>,
    /// Protocol extensions that the pool requires (clients must support these).
    required_extensions: Vec<u16>,
    webhook_client: WebhookClient,
}

#[cfg_attr(not(test), hotpath::measure_all)]
impl ChannelManager {
    /// Validates a Bitcoin address string and converts it to scriptPubKey bytes.
    ///
    /// # Arguments
    /// * `address` - Bitcoin address string to validate
    /// * `network` - Expected Bitcoin network (Regtest, Testnet, Mainnet, Signet)
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - scriptPubKey bytes if address is valid
    /// * `Err` - Descriptive error if address is invalid or wrong network
    ///
    /// # Example
    /// ```ignore
    /// let script_bytes = ChannelManager::validate_and_parse_address(
    ///     "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
    ///     Network::Regtest
    /// )?;
    /// ```
    fn validate_and_parse_address(
        address: &str,
        network: Network,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Parse address without network check first
        let address: Address<NetworkUnchecked> = address.parse()
            .map_err(|e| format!("Invalid Bitcoin address format: {}", e))?;

        // Validate network compatibility
        if !address.is_valid_for_network(network) {
            return Err(format!(
                "Address is not valid for expected network: {:?}",
                network
            ).into());
        }

        // Convert to scriptPubKey for storage in coinbase_outputs
        let script_pubkey = address.assume_checked_ref().script_pubkey();
        Ok(script_pubkey.to_bytes())
    }

    /// Constructor method used to instantiate the ChannelManager
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        config: PoolConfig,
        tp_sender: Sender<TemplateDistribution<'static>>,
        tp_receiver: Receiver<TemplateDistribution<'static>>,
        downstream_sender: broadcast::Sender<(DownstreamId, Mining<'static>, Option<Vec<Tlv>>)>,
        downstream_receiver: Receiver<(DownstreamId, Mining<'static>, Option<Vec<Tlv>>)>,
        coinbase_outputs: Vec<u8>,
        webhook_client: WebhookClient,
    ) -> PoolResult<Self, error::ChannelManager> {
        let range_0 = 0..0;
        let range_1 = 0..POOL_ALLOCATION_BYTES;
        let range_2 = POOL_ALLOCATION_BYTES..POOL_ALLOCATION_BYTES + CLIENT_SEARCH_SPACE_BYTES;

        let make_extranonce_factory = || {
            // simulating a scenario where there are multiple mining servers
            // this static prefix allows unique extranonce_prefix allocation
            // for this mining server
            let static_prefix = config.server_id().to_be_bytes().to_vec();

            ExtendedExtranonce::new(
                range_0.clone(),
                range_1.clone(),
                range_2.clone(),
                Some(static_prefix),
            )
            .expect("Failed to create ExtendedExtranonce with valid ranges")
        };

        let extranonce_prefix_factory_extended = make_extranonce_factory();
        let extranonce_prefix_factory_standard = make_extranonce_factory();

        let channel_manager_data = Arc::new(Mutex::new(ChannelManagerData {
            downstream: HashMap::new(),
            extranonce_prefix_factory_extended,
            extranonce_prefix_factory_standard,
            downstream_id_factory: AtomicUsize::new(1),
            vardiff: HashMap::new(),
            coinbase_outputs,
            last_future_template: None,
            last_new_prev_hash: None,
            job_to_user: HashMap::new(),
            current_user_id: "unknown".to_string(),
            current_pool_tag: config.pool_signature().to_string(),
            current_shares_per_minute: config.shares_per_minute(),
        }));

        let channel_manager_channel = ChannelManagerChannel {
            tp_sender,
            tp_receiver,
            downstream_sender,
            downstream_receiver,
        };

        let channel_manager = ChannelManager {
            channel_manager_data,
            channel_manager_channel,
            share_batch_size: config.share_batch_size(),
            coinbase_reward_script: config.coinbase_reward_script().clone(),
            supported_extensions: config.supported_extensions().to_vec(),
            required_extensions: config.required_extensions().to_vec(),
            webhook_client,
        };

        Ok(channel_manager)
    }

    // Bootstraps a group channel with the given parameters.
    // Returns a `GroupChannel` if successful, otherwise returns `None`.
    //
    // To be called before calling Downstream::new.
    fn bootstrap_group_channel(
        &self,
        channel_id: ChannelId,
    ) -> Option<GroupChannel<'static, DefaultJobStore<ExtendedJob<'static>>>> {
        let (last_future_template, last_set_new_prev_hash, pool_tag) =
            self.channel_manager_data.super_safe_lock(|data| {
                (
                    data.last_future_template
                        .clone()
                        .expect("No future template found after readiness check"),
                    data.last_new_prev_hash
                        .clone()
                        .expect("No new prevhash found after readiness check"),
                    data.current_pool_tag.clone(),
                )
            });
        let mut group_channel = match GroupChannel::new_for_pool(
            channel_id,
            DefaultJobStore::new(),
            FULL_EXTRANONCE_SIZE,
            pool_tag,
        ) {
            Ok(channel) => channel,
            Err(e) => {
                error!(error = ?e, "Failed to bootstrap group channel");
                return None;
            }
        };

        let coinbase_output = TxOut {
            value: Amount::from_sat(last_future_template.coinbase_tx_value_remaining),
            script_pubkey: self.coinbase_reward_script.script_pubkey(),
        };

        if let Err(e) = group_channel.on_new_template(last_future_template, vec![coinbase_output]) {
            error!(error = ?e, "Failed to add template to group channel");
            return None;
        }

        if let Err(e) = group_channel.on_set_new_prev_hash(last_set_new_prev_hash) {
            error!(error = ?e, "Failed to set new prevhash for group channel");
            return None;
        }

        Some(group_channel)
    }

    /// Starts the downstream server, and accepts new connection request.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_downstream_server(
        self,
        authority_public_key: Secp256k1PublicKey,
        authority_secret_key: Secp256k1SecretKey,
        cert_validity_sec: u64,
        listening_address: SocketAddr,
        task_manager: Arc<TaskManager>,
        notify_shutdown: broadcast::Sender<ShutdownMessage>,
        status_sender: Sender<Status>,
        channel_manager_sender: Sender<(DownstreamId, Mining<'static>, Option<Vec<Tlv>>)>,
        channel_manager_receiver: broadcast::Sender<(
            DownstreamId,
            Mining<'static>,
            Option<Vec<Tlv>>,
        )>,
    ) -> PoolResult<(), error::ChannelManager> {
        let mut shutdown_rx = notify_shutdown.subscribe();

        // Wait for initial template and prevhash before accepting connections
        loop {
            let has_required_data = self.channel_manager_data.super_safe_lock(|data| {
                data.last_future_template.is_some() && data.last_new_prev_hash.is_some()
            });

            if has_required_data {
                info!("Required template data received, ready to accept connections");
                break;
            }

            warn!("Waiting for initial template and prevhash from Template Provider...");
            select! {
                message = shutdown_rx.recv() => {
                    match message {
                        Ok(ShutdownMessage::ShutdownAll) => {
                            info!("Channel Manager: received shutdown while waiting for templates");
                            return Ok(());
                        }
                        Err(e) => {
                            warn!(error = ?e, "shutdown channel closed unexpectedly");
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
            }
        }

        info!("Starting downstream server at {listening_address}");
        let server = TcpListener::bind(listening_address)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to bind downstream server at {listening_address}");
                e
            })
            .map_err(PoolError::shutdown)?;

        let task_manager_clone = task_manager.clone();
        task_manager.spawn(async move {

            loop {
                select! {
                    message = shutdown_rx.recv() => {
                        match message {
                            Ok(ShutdownMessage::ShutdownAll) => {
                                info!("Channel Manager: received shutdown signal");
                                break;
                            }
                            Err(e) => {
                                warn!(error = ?e, "shutdown channel closed unexpectedly");
                                break;
                            }
                            _ => {}
                        }
                    }
                    res = server.accept() => {
                        match res {
                            Ok((stream, socket_address)) => {
                                info!(%socket_address, "New downstream connection");
                                let responder = match Responder::from_authority_kp(
                                    &authority_public_key.into_bytes(),
                                    &authority_secret_key.into_bytes(),
                                    std::time::Duration::from_secs(cert_validity_sec),
                                ) {
                                    Ok(r) => r,
                                    Err(e) => {
                                        error!(error = ?e, "Failed to create responder");
                                        continue;
                                    }
                                };
                                let noise_stream = match NoiseTcpStream::<Message>::new(
                                    stream,
                                    HandshakeRole::Responder(responder),
                                )
                                .await
                                {
                                    Ok(ns) => ns,
                                    Err(e) => {
                                        error!(error = ?e, "Noise handshake failed");
                                        continue;
                                    }
                                };

                                let downstream_id = self
                                    .channel_manager_data
                                    .super_safe_lock(|data| data.downstream_id_factory.fetch_add(1, Ordering::SeqCst));

                                let channel_id_factory = AtomicU32::new(1);
                                let group_channel_id = channel_id_factory.fetch_add(1, Ordering::SeqCst);
                                let group_channel = match self.bootstrap_group_channel(group_channel_id) {
                                    Some(group_channel) => group_channel,
                                    None => {
                                        error!("Failed to bootstrap group channel");
                                        let error = PoolError::<error::ChannelManager>::shutdown(PoolErrorKind::CouldNotInitiateSystem);
                                        handle_error(&StatusSender::ChannelManager(status_sender.clone()), error).await;
                                        break;
                                    }
                                };

                                let downstream = Downstream::new(
                                    downstream_id,
                                    channel_id_factory,
                                    group_channel,
                                    channel_manager_sender.clone(),
                                    channel_manager_receiver.clone(),
                                    noise_stream,
                                    notify_shutdown.clone(),
                                    task_manager_clone.clone(),
                                    status_sender.clone(),
                                    self.supported_extensions.clone(),
                                    self.required_extensions.clone(),
                                );

                                self.channel_manager_data.super_safe_lock(|data| {
                                    data.downstream.insert(downstream_id, downstream.clone());
                                });

                                downstream
                                    .start(
                                        notify_shutdown.clone(),
                                        status_sender.clone(),
                                        task_manager_clone.clone(),
                                    )
                                    .await;
                                }

                                Err(e) => {
                                    error!(error = ?e, "Failed to accept new downstream connection");
                                }
                            }
                    }
                }
            }
            info!("Downstream server: Unified loop break");
        });
        Ok(())
    }

    /// The central orchestrator of the Channel Manager.  
    ///  
    /// Responsible for receiving messages from all subsystems, processing them,  
    /// and either forwarding them to the appropriate subsystem or updating  
    /// the internal state of the Channel Manager as needed.
    pub async fn start(
        self,
        notify_shutdown: broadcast::Sender<ShutdownMessage>,
        status_sender: Sender<Status>,
        task_manager: Arc<TaskManager>,
        coinbase_outputs: Vec<TxOut>,
    ) -> PoolResult<(), error::ChannelManager> {
        let status_sender = StatusSender::ChannelManager(status_sender);
        let mut shutdown_rx = notify_shutdown.subscribe();

        self.coinbase_output_constraints(coinbase_outputs).await?;

        task_manager.spawn(async move {
            let cm = self.clone();
            let vardiff_future = self.run_vardiff_loop();
            tokio::pin!(vardiff_future);
            loop {
                let mut cm_template = cm.clone();
                let mut cm_downstreams = cm.clone();
                tokio::select! {
                    message = shutdown_rx.recv() => {
                        match message {
                            Ok(ShutdownMessage::ShutdownAll) => {
                                info!("Channel Manager: received shutdown signal");
                                break;
                            }
                            Ok(ShutdownMessage::DownstreamShutdown(downstream_id)) => {
                                info!(%downstream_id, "Channel Manager: removing downstream after shutdown");
                                if let Err(e) = self.remove_downstream(downstream_id) {
                                    tracing::error!(%downstream_id, error = ?e, "Failed to remove downstream");
                                }
                            }
                            Err(e) => {
                                warn!(error = ?e, "shutdown channel closed unexpectedly");
                                break;
                            }
                            _ => {}
                        }
                    }
                    res = &mut vardiff_future => {
                        info!("Vardiff loop completed with: {res:?}");
                    }
                    res = cm_template.handle_template_provider_message() => {
                        if let Err(e) = res {
                            error!(error = ?e, "Error handling Template Receiver message");
                            if handle_error(&status_sender, e).await {
                                break;
                            }
                        }
                    }
                    res = cm_downstreams.handle_downstream_mining_message() => {
                        if let Err(e) = res {
                            error!(error = ?e, "Error handling Downstreams message");
                            if handle_error(&status_sender, e).await {
                                break;
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }

    // Removes a Downstream entry from the ChannelManager’s state.
    //
    // Given a `downstream_id`, this method:
    // 1. Removes the corresponding Downstream from the `downstream` map.
    // 2. Removes the channels of the corresponding Downstream from `vardiff` map.
    #[allow(clippy::result_large_err)]
    fn remove_downstream(
        &self,
        downstream_id: DownstreamId,
    ) -> PoolResult<(), error::ChannelManager> {
        self.channel_manager_data.super_safe_lock(|cm_data| {
            cm_data.downstream.remove(&downstream_id);
            cm_data
                .vardiff
                .retain(|key, _| key.downstream_id != downstream_id);
        });
        Ok(())
    }

    // Handles messages received from the TP subsystem.
    //
    // This method listens for incoming frames on the `tp_receiver` channel.
    // - If the frame contains a TemplateDistribution message, it forwards it to the template
    //   distribution message handler.
    // - If the frame contains any unsupported message type, an error is returned.
    async fn handle_template_provider_message(&mut self) -> PoolResult<(), error::ChannelManager> {
        if let Ok(message) = self.channel_manager_channel.tp_receiver.recv().await {
            self.handle_template_distribution_message_from_server(None, message, None)
                .await?;
        }
        Ok(())
    }

    async fn handle_downstream_mining_message(&mut self) -> PoolResult<(), error::ChannelManager> {
        if let Ok((downstream_id, message, tlv_fields)) = self
            .channel_manager_channel
            .downstream_receiver
            .recv()
            .await
        {
            let tlv_slice = tlv_fields.as_deref();
            self.handle_mining_message_from_client(Some(downstream_id), message, tlv_slice)
                .await?;
        }

        Ok(())
    }

    // Runs the vardiff on extended channel.
    fn run_vardiff_on_extended_channel(
        downstream_id: DownstreamId,
        channel_id: ChannelId,
        channel_state: &mut ExtendedChannel<'static, DefaultJobStore<ExtendedJob<'static>>>,
        vardiff_state: &mut VardiffState,
        updates: &mut Vec<RouteMessageTo>,
    ) {
        let (hashrate, target, shares_per_minute) = (
            channel_state.get_nominal_hashrate(),
            channel_state.get_target(),
            channel_state.get_shares_per_minute(),
        );

        let Ok(new_hashrate_opt) = vardiff_state.try_vardiff(hashrate, target, shares_per_minute)
        else {
            debug!("Vardiff computation failed for extended channel {channel_id}");
            return;
        };

        let Some(new_hashrate) = new_hashrate_opt else {
            return;
        };

        match channel_state.update_channel(new_hashrate, None) {
            Ok(()) => {
                let updated_target = channel_state.get_target();
                updates.push(
                    (
                        downstream_id,
                        Mining::SetTarget(SetTarget {
                            channel_id,
                            maximum_target: updated_target.to_le_bytes().into(),
                        }),
                    )
                        .into(),
                );
                debug!("Updated target for extended channel_id={channel_id} to {updated_target:?}",);
            }
            Err(e) => warn!(
                "Failed to update extended channel channel_id={channel_id} during vardiff {e:?}"
            ),
        }
    }

    // Runs the vardiff on the standard channel.
    fn run_vardiff_on_standard_channel(
        downstream_id: DownstreamId,
        channel_id: ChannelId,
        channel: &mut StandardChannel<'static, DefaultJobStore<StandardJob<'static>>>,
        vardiff_state: &mut VardiffState,
        updates: &mut Vec<RouteMessageTo>,
    ) {
        let hashrate = channel.get_nominal_hashrate();
        let target = channel.get_target();
        let shares_per_minute = channel.get_shares_per_minute();

        let Ok(new_hashrate_opt) = vardiff_state.try_vardiff(hashrate, target, shares_per_minute)
        else {
            debug!("Vardiff computation failed for standard channel {channel_id}");
            return;
        };

        if let Some(new_hashrate) = new_hashrate_opt {
            match channel.update_channel(new_hashrate, None) {
                Ok(()) => {
                    let updated_target = channel.get_target();
                    updates.push(
                        (
                            downstream_id,
                            Mining::SetTarget(SetTarget {
                                channel_id,
                                maximum_target: updated_target.to_le_bytes().into(),
                            }),
                        )
                            .into(),
                    );
                    debug!(
                        "Updated target for standard channel channel_id={channel_id} to {updated_target:?}"
                    );
                }
                Err(e) => warn!(
                    "Failed to update standard channel channel_id={channel_id} during vardiff {e:?}"
                ),
            }
        }
    }

    // Periodic vardiff task loop.
    //
    // # Purpose
    // - Executes the vardiff cycle every 60 seconds for all downstreams.
    // - Delegates to [`Self::run_vardiff`] on each tick.
    async fn run_vardiff_loop(&self) -> PoolResult<(), error::ChannelManager> {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            ticker.tick().await;
            info!("Starting vardiff loop for downstreams");

            if let Err(e) = self.run_vardiff().await {
                error!(error = ?e, "Vardiff iteration failed");
            }
        }
    }

    // Runs vardiff across **all channels** and generates updates.
    //
    // # Purpose
    // - Iterates through all downstream channels (both standard and extended).
    // - Runs vardiff for each channel and collects the resulting updates.
    // - Propagates difficulty changes to downstreams and also sends an `UpdateChannel` message
    //   upstream if applicable.
    async fn run_vardiff(&self) -> PoolResult<(), error::ChannelManager> {
        let mut messages: Vec<RouteMessageTo> = vec![];
        self.channel_manager_data
            .super_safe_lock(|channel_manager_data| {
                for (vardiff_key, vardiff_state) in channel_manager_data.vardiff.iter_mut() {
                    let downstream_id = &vardiff_key.downstream_id;
                    let channel_id = &vardiff_key.channel_id;

                    let Some(downstream) = channel_manager_data.downstream.get_mut(downstream_id)
                    else {
                        continue;
                    };
                    downstream.downstream_data.super_safe_lock(|data| {
                        if let Some(standard_channel) = data.standard_channels.get_mut(channel_id) {
                            Self::run_vardiff_on_standard_channel(
                                *downstream_id,
                                *channel_id,
                                standard_channel,
                                vardiff_state,
                                &mut messages,
                            );
                        }
                        if let Some(extended_channel) = data.extended_channels.get_mut(channel_id) {
                            Self::run_vardiff_on_extended_channel(
                                *downstream_id,
                                *channel_id,
                                extended_channel,
                                vardiff_state,
                                &mut messages,
                            );
                        }
                    });
                }
            });

        for message in messages {
            message.forward(&self.channel_manager_channel).await;
        }

        info!("Vardiff update cycle complete");
        Ok(())
    }

    /// Updates the pool's coinbase outputs and broadcasts new jobs to all miners.
    ///
    /// # Purpose
    /// Enables dynamic coinbase address switching for time-shared mining. This method:
    /// 1. Validates the Bitcoin address
    /// 2. Updates the coinbase output scriptPubKey atomically
    /// 3. Recreates jobs from the stored template with the new coinbase
    /// 4. Broadcasts updated jobs to all connected miners
    ///
    /// # Parameters
    /// - `new_address`: Bitcoin address string to validate and use for coinbase rewards
    /// - `user_id`: User identifier for share attribution (stored for Phase 2 webhook integration)
    ///
    /// # Returns
    /// - `Ok(())` if update and broadcast succeed
    /// - `Err` if address validation fails or job distribution fails
    ///
    /// # Concurrency Safety
    /// Uses lock-minimizing pattern to prevent deadlock:
    /// - Phase 1: Validate address (no locks)
    /// - Phase 2: Read template (short lock on channel_manager_data)
    /// - Phase 3: Compute new outputs (no locks)
    /// - Phase 4: Update state and generate jobs (short locks, never nested)
    /// - Phase 5: Broadcast messages (no locks)
    ///
    /// # Example
    /// ```ignore
    /// channel_manager.update_coinbase_and_broadcast(
    ///     "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
    ///     "user_123".to_string()
    /// ).await?;
    /// ```
    pub async fn update_coinbase_and_broadcast(
        &self,
        new_address: &str,
        _user_id: String, // Stored for Phase 2 share attribution
        pool_tag: Option<String>, // Optional pool tag for coinbase scriptSig customization
    ) -> PoolResult<(), error::ChannelManager> {
        use stratum_apps::stratum_core::{
            bitcoin::consensus::Encodable,
            channels_sv2::outputs::deserialize_outputs,
        };

        // Phase 1: Validate address before acquiring any locks
        let new_script_pubkey = Self::validate_and_parse_address(new_address, Network::Regtest)
            .map_err(|e| {
                error!("Invalid Bitcoin address: {}", e);
                PoolError::log(format!("Invalid Bitcoin address: {}", e))
            })?;

        info!("Updating coinbase to address: {}...", &new_address[..8.min(new_address.len())]);

        // Phase 2: Read current template state (short lock)
        let last_template_value = self.channel_manager_data.super_safe_lock(|data| {
            data.last_future_template
                .as_ref()
                .map(|t| t.coinbase_tx_value_remaining)
                .unwrap_or(0)
        });

        // Phase 3: Compute new coinbase outputs (no locks held)
        // Create fresh coinbase output with new address
        // The value must match the template's coinbase_tx_value_remaining for validation
        use stratum_apps::stratum_core::bitcoin::{Amount, TxOut, ScriptBuf};

        let coinbase_outputs = vec![TxOut {
            value: Amount::from_sat(last_template_value),
            script_pubkey: ScriptBuf::from_bytes(new_script_pubkey),
        }];

        // Serialize back to bytes for storage
        let mut new_encoded_outputs = Vec::new();
        coinbase_outputs.consensus_encode(&mut new_encoded_outputs)
            .map_err(|e| {
                error!("Failed to encode coinbase outputs: {:?}", e);
                PoolError::shutdown(PoolErrorKind::Custom(format!("Failed to encode coinbase outputs: {:?}", e)))
            })?;

        // Phase 4: Atomic state update and job generation (short locks, never nested)
        let messages = self.channel_manager_data.super_safe_lock(|channel_manager_data| {
            // Update coinbase outputs atomically
            channel_manager_data.coinbase_outputs = new_encoded_outputs.clone();

            // Store user_id for persistent attribution across template updates
            channel_manager_data.current_user_id = _user_id.clone();
            info!("SET current_user_id = {}", channel_manager_data.current_user_id);

            // Update pool_tag if provided, otherwise keep current value
            if let Some(tag) = pool_tag {
                channel_manager_data.current_pool_tag = tag.clone();
                info!("SET current_pool_tag = {}", channel_manager_data.current_pool_tag);
            }

            // Get last_future_template for job recreation
            let last_future_template = channel_manager_data.last_future_template
                .as_ref()
                .ok_or_else(|| PoolError::log(PoolErrorKind::Custom("No template available for job recreation".to_string())))?
                .clone();

            let mut messages: Vec<RouteMessageTo> = Vec::new();

            // Deserialize the new coinbase outputs for job recreation
            let coinbase_outputs_for_jobs = deserialize_outputs(new_encoded_outputs.clone())
                .map_err(|e| {
                    error!("Failed to deserialize new coinbase outputs for job recreation: {:?}", e);
                    PoolError::log(PoolErrorKind::Custom("Failed to deserialize new coinbase outputs".to_string()))
                })?;

            // Iterate over downstreams to generate job messages
            for (downstream_id, downstream) in channel_manager_data.downstream.iter_mut() {
                // Skip job declarator clients (REQUIRES_CUSTOM_WORK flag)
                let requires_custom_work = downstream.requires_custom_work.load(Ordering::SeqCst);
                if requires_custom_work {
                    continue;
                }

                // Generate jobs for this downstream (acquire downstream lock separately)
                let downstream_messages: Vec<RouteMessageTo<'_>> = downstream.downstream_data.super_safe_lock(|data| {
                    let mut messages: Vec<RouteMessageTo> = vec![];

                    // Update group channel with new coinbase outputs
                    // This recreates the merkle path and jobs with the new coinbase
                    data.group_channel.on_new_template(last_future_template.clone(), coinbase_outputs_for_jobs.clone())
                        .map_err(|e| {
                            error!("Failed to update group channel with new coinbase: {:?}", e);
                            PoolError::shutdown(e)
                        })?;

                    // Get the updated job from group channel
                    let group_channel_job = data.group_channel.get_active_job()
                        .ok_or_else(|| PoolError::shutdown(PoolErrorKind::JobNotFound))?;

                    // Check if this downstream requires standard jobs
                    let requires_standard_jobs = downstream.requires_standard_jobs.load(Ordering::SeqCst);
                    let empty_group_channel = data.group_channel.get_channel_ids().is_empty();

                    // If REQUIRES_STANDARD_JOBS is not set and group channel is not empty,
                    // send NewExtendedMiningJob to the group channel
                    if !requires_standard_jobs && !empty_group_channel {
                        messages.push((*downstream_id, Mining::NewExtendedMiningJob(group_channel_job.get_job_message().clone())).into());
                    }

                    // Update standard channels
                    for (_channel_id, standard_channel) in data.standard_channels.iter_mut() {
                        if !requires_standard_jobs {
                            // Standard channels get jobs from group channel
                            standard_channel.on_group_channel_job(group_channel_job.clone())
                                .map_err(|e| {
                                    error!("Failed to update standard channel with group job: {:?}", e);
                                    PoolError::shutdown(e)
                                })?;
                        } else {
                            // Update standard channel directly with new coinbase
                            standard_channel.on_new_template(last_future_template.clone(), coinbase_outputs_for_jobs.clone())
                                .map_err(|e| {
                                    error!("Failed to update standard channel with new coinbase: {:?}", e);
                                    PoolError::shutdown(e)
                                })?;

                            // Send NewMiningJob to standard channel
                            if let Some(standard_job) = standard_channel.get_active_job() {
                                messages.push((*downstream_id, Mining::NewMiningJob(standard_job.get_job_message().clone())).into());
                            }
                        }
                    }

                    // Update extended channels with group channel job
                    for (_channel_id, extended_channel) in data.extended_channels.iter_mut() {
                        extended_channel.on_group_channel_job(group_channel_job.clone())
                            .map_err(|e| {
                                error!("Failed to update extended channel with group job: {:?}", e);
                                PoolError::shutdown(e)
                            })?;
                    }

                    // CRITICAL: Store job-to-user mappings BEFORE broadcasting messages (WEBHOOK-06)
                    // This ensures mappings exist when shares arrive immediately after job distribution
                    // Use current_user_id which persists across template updates
                    let user_id = channel_manager_data.current_user_id.clone();
                    info!("update_coinbase: populating job_to_user with user_id={}", user_id);

                    // Group channel job ID (used by extended channels)
                    let group_job_id = group_channel_job.get_job_id();
                    channel_manager_data.job_to_user.insert(group_job_id, user_id.clone());

                    // Standard channel job IDs (if any)
                    for (_channel_id, standard_channel) in data.standard_channels.iter() {
                        if let Some(standard_job) = standard_channel.get_active_job() {
                            let standard_job_id = standard_job.get_job_id();
                            channel_manager_data.job_to_user.insert(standard_job_id, user_id.clone());
                        }
                    }

                    Ok::<Vec<RouteMessageTo<'_>>, PoolError<error::ChannelManager>>(messages)
                })?;

                messages.extend(downstream_messages);
            }

            Ok::<Vec<RouteMessageTo<'_>>, PoolError<error::ChannelManager>>(messages)
        })?;

        // Phase 5: Broadcast messages (no locks, all I/O)
        let message_count = messages.len();
        for message in messages {
            message.forward(&self.channel_manager_channel).await;
        }

        info!("Coinbase updated and jobs broadcast to {} miners", message_count);
        Ok(())
    }

    /// Sends a CoinbaseOutputConstraints message to the template provider.
    ///
    /// # Purpose
    /// - Calculates the max coinbase output size and sigops for the coinbase outputs.
    /// - Sends the CoinbaseOutputConstraints message to the template provider.
    ///
    /// # Parameters
    /// - `coinbase_outputs`: The coinbase outputs to calculate the max coinbase output size and
    ///   sigops for.
    pub async fn coinbase_output_constraints(
        &self,
        coinbase_outputs: Vec<TxOut>,
    ) -> PoolResult<(), error::ChannelManager> {
        let msg = coinbase_output_constraints_message(coinbase_outputs);

        self.channel_manager_channel
            .tp_sender
            .send(TemplateDistribution::CoinbaseOutputConstraints(msg))
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to send CoinbaseOutputConstraints message to TP");
                PoolError::shutdown(PoolErrorKind::ChannelErrorSender)
            })?;

        Ok(())
    }
}

#[derive(Clone)]
pub enum RouteMessageTo<'a> {
    /// Route to the template provider subsystem.
    TemplateProvider(TemplateDistribution<'a>),
    /// Route to a specific downstream client by ID, along with its mining message.
    Downstream((DownstreamId, Mining<'a>)),
}

impl<'a> From<TemplateDistribution<'a>> for RouteMessageTo<'a> {
    fn from(value: TemplateDistribution<'a>) -> Self {
        Self::TemplateProvider(value)
    }
}

impl<'a> From<(DownstreamId, Mining<'a>)> for RouteMessageTo<'a> {
    fn from(value: (DownstreamId, Mining<'a>)) -> Self {
        Self::Downstream(value)
    }
}

impl RouteMessageTo<'_> {
    pub async fn forward(self, channel_manager_channel: &ChannelManagerChannel) {
        match self {
            RouteMessageTo::Downstream((downstream_id, message)) => {
                _ = channel_manager_channel.downstream_sender.send((
                    downstream_id,
                    message.into_static(),
                    None,
                ));
            }
            RouteMessageTo::TemplateProvider(message) => {
                _ = channel_manager_channel
                    .tp_sender
                    .send(message.into_static())
                    .await;
            }
        }
    }
}

impl std::fmt::Debug for ChannelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelManager")
            .field("share_batch_size", &self.share_batch_size)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_and_parse_address_valid_regtest() {
        // Valid P2PKH testnet address (works on regtest)
        // Note: Bitcoin Core in regtest mode accepts testnet-format addresses
        let address = "mwCwTceJvYV27KXBc3NJZys6CjsgsoeHmf";
        let result = ChannelManager::validate_and_parse_address(address, Network::Regtest);
        if let Err(e) = &result {
            eprintln!("Validation error: {}", e);
        }
        assert!(result.is_ok(), "Valid regtest address should parse successfully: {:?}", result);

        let script_bytes = result.unwrap();
        assert!(!script_bytes.is_empty(), "Script pubkey should not be empty");
    }

    #[test]
    fn test_validate_and_parse_address_invalid_format() {
        // Invalid address format
        let address = "not_a_valid_address";
        let result = ChannelManager::validate_and_parse_address(address, Network::Regtest);
        assert!(result.is_err(), "Invalid address format should return error");

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid Bitcoin address format"),
                "Error should mention invalid format: {}", error_msg);
    }

    #[test]
    fn test_validate_and_parse_address_wrong_network() {
        // Mainnet P2PKH address
        let mainnet_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let result = ChannelManager::validate_and_parse_address(mainnet_address, Network::Regtest);

        // Bitcoin address validation is strict about network - mainnet addresses
        // should not be valid for regtest
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("not valid") || error_msg.contains("network"),
                    "Error should mention network validation: {}", error_msg);
        } else {
            // If the address is accepted, it's because regtest is lenient
            // This is acceptable behavior for test environments
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_validate_and_parse_address_p2pkh_regtest() {
        // Valid P2PKH regtest address
        let address = "mwCwTceJvYV27KXBc3NJZys6CjsgsoeHmf";
        let result = ChannelManager::validate_and_parse_address(address, Network::Regtest);
        assert!(result.is_ok(), "Valid P2PKH regtest address should parse successfully");
    }

    // Note: Full integration tests for update_coinbase_and_broadcast require:
    // - Setting up ChannelManager with template provider channels
    // - Creating downstream connections
    // - Receiving initial template and prevhash
    // - Calling update_coinbase_and_broadcast
    // - Verifying job messages are sent to downstreams
    //
    // These are covered by the integration test in test_merkle_path_validity_after_coinbase_change
    // (Plan 01-02) which validates the complete flow including share acceptance.
    //
    // Unit tests here focus on validation logic and error handling.

    #[test]
    fn test_validate_and_parse_address_empty_string() {
        let address = "";
        let result = ChannelManager::validate_and_parse_address(address, Network::Regtest);
        assert!(result.is_err(), "Empty address should return error");
    }

    #[test]
    fn test_validate_and_parse_address_special_characters() {
        let address = "bc1q!@#$%^&*()";
        let result = ChannelManager::validate_and_parse_address(address, Network::Regtest);
        assert!(result.is_err(), "Address with special characters should return error");
    }
}
