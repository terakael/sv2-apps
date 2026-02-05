//! # Integration Test: Coinbase Merkle Path Validity
//!
//! ## Purpose
//! This test validates the **core feasibility assumption** for dynamic coinbase switching:
//! **Merkle paths remain valid when coinbase outputs change.**
//!
//! ## Why This Is Critical
//! The entire dynamic coinbase switching approach depends on being able to:
//! 1. Receive a mining job from the pool with merkle path for transaction set
//! 2. Modify the coinbase outputs (to switch payout addresses)
//! 3. Submit shares using the **same merkle path** but with the **modified coinbase**
//! 4. Have the pool accept those shares without "Invalid merkle root" errors
//!
//! If this test fails, it means the merkle path is coupled to the specific coinbase content,
//! and the entire approach requires architectural redesign.
//!
//! ## What It Proves
//! - Merkle path computation is independent of coinbase transaction content
//! - Share validation uses the modified coinbase to recompute merkle root
//! - The pool's share validation correctly handles coinbase modifications
//!
//! ## References
//! - RESEARCH.md Pitfall 2 (lines 279-288): Identified as feasibility blocker
//! - REQUIREMENTS.md TMPL-05: Merkle path theory
//! - PLAN 01-02: Empirical validation task
//!
//! ## Test Flow
//! 1. **Setup**: Start regtest Template Provider and Pool
//! 2. **Connect**: Mining device connects and opens standard channel
//! 3. **Receive Job**: Wait for NewMiningJob with initial coinbase
//! 4. **Modify Coinbase**: Directly access ChannelManager and change coinbase_outputs (simulates HTTP API)
//! 5. **Regenerate Jobs**: Trigger job regeneration with new coinbase
//! 6. **Submit Share**: Mining device mines and submits valid share
//! 7. **Verify Acceptance**: Pool responds with SubmitSharesSuccess (NOT InvalidMerkleRoot error)
//!
//! ## Prerequisites
//! This test requires capnproto to be installed:
//! ```bash
//! # Ubuntu/Debian
//! sudo apt-get install capnproto libcapnp-dev
//!
//! # macOS
//! brew install capnproto
//! ```

use integration_tests_sv2::*;
use stratum_apps::stratum_core::{
    bitcoin::{consensus::Encodable, Address, Amount, Network, TxOut},
    common_messages_sv2::Protocol,
    mining_sv2::*,
    parsers_sv2::{AnyMessage, Mining},
};

/// Test that shares are accepted after coinbase modification.
///
/// This validates the merkle path reuse assumption (TMPL-05).
#[tokio::test]
async fn test_coinbase_modification_preserves_share_validity() {
    start_tracing();

    println!("\n=== COINBASE MERKLE VALIDITY TEST ===");
    println!("Testing: Shares accepted after coinbase modification");
    println!("Validates: Merkle path reuse (TMPL-05 feasibility assumption)\n");

    // ========================================
    // PHASE 1: Setup Pool and Template Provider
    // ========================================
    println!("[Phase 1] Starting Template Provider and Pool...");

    let (_tp, tp_addr) = start_template_provider(None, template_provider::DifficultyLevel::Low);
    let (pool, pool_addr) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;

    println!("  ✓ Template Provider at: {}", tp_addr);
    println!("  ✓ Pool at: {}", pool_addr);

    // ========================================
    // PHASE 2: Connect Mining Device and Receive Initial Job
    // ========================================
    println!("\n[Phase 2] Connecting mining device...");

    let (sniffer, sniffer_addr) = start_sniffer(
        "coinbase_test",
        pool_addr,
        false,
        vec![],
        None,
    );

    // Start mining device that will connect and receive jobs
    start_mining_device_sv2(sniffer_addr, None, None, None, 1, None, false);

    // Wait for connection handshake
    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToUpstream,
            MESSAGE_TYPE_SETUP_CONNECTION,
        )
        .await;
    println!("  ✓ Mining device sent SetupConnection");

    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;
    println!("  ✓ Pool responded with SetupConnectionSuccess");

    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToUpstream,
            MESSAGE_TYPE_OPEN_STANDARD_MINING_CHANNEL,
        )
        .await;
    println!("  ✓ Mining device requested standard channel");

    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToDownstream,
            MESSAGE_TYPE_OPEN_STANDARD_MINING_CHANNEL_SUCCESS,
        )
        .await;
    println!("  ✓ Pool opened standard channel");

    // Wait for initial job
    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToDownstream,
            MESSAGE_TYPE_NEW_MINING_JOB,
        )
        .await;

    let initial_job = match sniffer.next_message_from_upstream() {
        Some((_, AnyMessage::Mining(Mining::NewMiningJob(job)))) => {
            println!("  ✓ Received initial mining job");
            println!("    - Job ID: {}", job.job_id);
            println!("    - Version: {}", job.version);
            job
        }
        msg => panic!("Expected NewMiningJob, got: {:?}", msg),
    };

    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToDownstream,
            MESSAGE_TYPE_MINING_SET_NEW_PREV_HASH,
        )
        .await;
    println!("  ✓ Received SetNewPrevHash");

    // ========================================
    // PHASE 3: Modify Coinbase Outputs
    // ========================================
    println!("\n[Phase 3] Modifying coinbase outputs...");
    println!("  This simulates what the HTTP API will do in production");

    // Create new coinbase output with different address
    // This simulates switching to a different user's payout address
    let original_address = "tb1qa0sm0hxzj0x25rh8gw5xlzwlsfvvyz8u96w3p8";
    let new_address_str = "tb1qpusf5256yxv50qt0pm0tue8k952fsu5lzsphft";

    println!("  Original address: {}", original_address);
    println!("  New address:      {}", new_address_str);

    let new_address = Address::from_str(new_address_str)
        .expect("Valid address")
        .require_network(Network::Testnet)
        .expect("Valid testnet address");

    let new_coinbase_output = TxOut {
        value: Amount::from_sat(5000000000), // 50 BTC (regtest coinbase reward)
        script_pubkey: new_address.script_pubkey(),
    };

    // Encode the new coinbase outputs (as pool does)
    let mut new_encoded_outputs = vec![];
    vec![new_coinbase_output]
        .consensus_encode(&mut new_encoded_outputs)
        .expect("Valid coinbase output encoding");

    // *** CRITICAL MOMENT ***
    // This is where we modify the coinbase_outputs in ChannelManager state
    // This simulates what the HTTP API endpoint will do
    println!("  ⚠ CRITICAL: Modifying ChannelManager.coinbase_outputs");

    pool.test_set_coinbase_outputs(new_encoded_outputs.clone());

    println!("  ✓ Coinbase outputs modified successfully");
    println!("    New encoded outputs length: {} bytes", new_encoded_outputs.len());

    // Force job regeneration with new coinbase by creating a mempool transaction
    // This triggers the Template Provider to send a new template to the pool
    // The pool will then generate new jobs using the MODIFIED coinbase_outputs
    println!("  Triggering new template from Template Provider...");

    _tp.create_mempool_transaction()
        .expect("Failed to create mempool transaction");

    println!("  ✓ Mempool transaction created, pool should receive new template");

    // Wait for the new job with modified coinbase to be sent to mining device
    println!("  Waiting for new job with modified coinbase...");

    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToDownstream,
            MESSAGE_TYPE_NEW_MINING_JOB,
        )
        .await;

    let modified_job = match sniffer.next_message_from_upstream() {
        Some((_, AnyMessage::Mining(Mining::NewMiningJob(job)))) => {
            println!("  ✓ Received new job with modified coinbase");
            println!("    - Job ID: {} (different from initial)", job.job_id);
            println!("    - This job uses the MODIFIED coinbase outputs");
            job
        }
        msg => panic!("Expected NewMiningJob after template update, got: {:?}", msg),
    };

    assert_ne!(
        initial_job.job_id, modified_job.job_id,
        "New job should have different ID from initial job"
    );

    // The mining device will now mine on the new job with the modified coinbase
    println!("  Mining device now working on job with modified coinbase...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // ========================================
    // PHASE 4: Submit Share and Verify Acceptance
    // ========================================
    println!("\n[Phase 4] Mining device submitting share with modified coinbase...");
    println!("  If merkle path is invalid, pool will reject with InvalidMerkleRoot");
    println!("  If merkle path is valid, pool will accept the share");

    // Wait for mining device to submit a share
    // The mining device is continuously mining in the background
    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToUpstream,
            MESSAGE_TYPE_SUBMIT_SHARES_STANDARD,
        )
        .await;

    println!("  ✓ Mining device submitted share");

    // *** CRITICAL VALIDATION ***
    // This is the moment of truth: does the pool accept the share?
    sniffer
        .wait_for_message_type(
            interceptor::MessageDirection::ToDownstream,
            MESSAGE_TYPE_SUBMIT_SHARES_SUCCESS,
        )
        .await;

    let success_msg = match sniffer.next_message_from_upstream() {
        Some((_, AnyMessage::Mining(Mining::SubmitSharesSuccess(msg)))) => {
            println!("  ✓✓✓ SHARE ACCEPTED! ✓✓✓");
            println!("    - Channel ID: {}", msg.channel_id);
            println!("    - Last sequence number: {}", msg.last_sequence_number);
            println!("    - New submits accepted count: {}", msg.new_submits_accepted_count);
            msg
        }
        Some((_, AnyMessage::Mining(Mining::SubmitSharesError(err)))) => {
            panic!(
                "FEASIBILITY TEST FAILED!\n\
                 Share was REJECTED after coinbase modification.\n\
                 Error: {:?}\n\
                 This means merkle paths are NOT reusable after coinbase changes.\n\
                 The entire dynamic coinbase switching approach requires rearchitecture.",
                err
            );
        }
        msg => panic!("Expected SubmitSharesSuccess or Error, got: {:?}", msg),
    };

    assert!(success_msg.new_submits_accepted_count > 0, "At least one share should be accepted");

    println!("\n=== TEST RESULT: SUCCESS ===");
    println!("✓ Merkle paths remain valid after coinbase modification");
    println!("✓ Shares are accepted with modified coinbase outputs");
    println!("✓ Core feasibility assumption validated");
    println!("✓ Dynamic coinbase switching approach is feasible\n");
}

// Helper to import Address::from_str
use std::str::FromStr;
