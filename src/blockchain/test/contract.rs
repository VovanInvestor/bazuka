use super::*;
use std::str::FromStr;

#[test]
fn test_contract_create_patch() -> Result<(), BlockchainError> {
    let miner = Wallet::new(Vec::from("MINER"));
    let alice = Wallet::new(Vec::from("ABC"));
    let mut chain = KvStoreChain::new(db::RamKvStore::new(), easy_config())?;

    let state_model = zk::ZkStateModel::List {
        item_type: Box::new(zk::ZkStateModel::Scalar),
        log4_size: 5,
    };
    let full_state = zk::ZkState {
        rollbacks: vec![],
        data: Default::default(),
    };

    let tx = alice.create_contract(
        zk::ZkContract {
            state_model: state_model.clone(),
            initial_state: state_model.compress::<ZkHasher>(&full_state.data)?,
            payment_functions: Vec::new(),
            functions: Vec::new(),
        },
        full_state.data.clone(),
        Money(0),
        1,
    );

    let draft = chain
        .draft_block(1, &with_dummy_stats(&[tx.clone()]), &miner, true)?
        .unwrap();
    chain.apply_block(&draft.block, true)?;

    assert_eq!(chain.get_height()?, 2);
    assert_eq!(chain.get_outdated_contracts()?.len(), 1);

    chain.update_states(&draft.patch)?;

    assert_eq!(chain.get_height()?, 2);
    assert_eq!(chain.get_outdated_contracts()?.len(), 0);

    rollback_till_empty(&mut chain)?;

    Ok(())
}

#[test]
fn test_contract_update() -> Result<(), BlockchainError> {
    let miner = Wallet::new(Vec::from("MINER"));
    let alice = Wallet::new(Vec::from("ABC"));
    let cid =
        ContractId::from_str("94f768758eebc1e0a1fc806726db01aaff5331763ce7c93b253770abfa7a53ee")
            .unwrap();
    let mut chain = KvStoreChain::new(db::RamKvStore::new(), easy_config())?;

    let state_model = zk::ZkStateModel::List {
        item_type: Box::new(zk::ZkStateModel::Scalar),
        log4_size: 5,
    };
    let mut full_state = zk::ZkState {
        rollbacks: vec![],
        data: zk::ZkDataPairs(
            [(zk::ZkDataLocator(vec![100]), zk::ZkScalar::from(200))]
                .into_iter()
                .collect(),
        ),
    };
    let mut full_state_with_delta = full_state.clone();

    let state_delta = zk::ZkDeltaPairs(
        [(zk::ZkDataLocator(vec![123]), Some(zk::ZkScalar::from(234)))]
            .into_iter()
            .collect(),
    );
    full_state.apply_delta(&state_delta);
    full_state_with_delta.push_delta(&state_delta);

    let tx = alice.call_function(
        cid,
        0,
        state_delta.clone(),
        state_model.compress::<ZkHasher>(&full_state.data)?,
        zk::ZkProof::Dummy(true),
        Money(0),
        Money(0),
        1,
    );

    let draft = chain
        .draft_block(1, &with_dummy_stats(&[tx.clone()]), &miner, false)?
        .unwrap();

    chain.apply_block(&draft.block, true)?;

    assert!(matches!(
        chain.fork_on_ram().update_states(&ZkBlockchainPatch {
            patches: HashMap::new()
        }),
        Err(BlockchainError::FullStateNotFound)
    ));
    assert!(matches!(
        chain.fork_on_ram().update_states(&ZkBlockchainPatch {
            patches: [(
                cid,
                zk::ZkStatePatch::Delta(zk::ZkDeltaPairs(
                    [(zk::ZkDataLocator(vec![123]), Some(zk::ZkScalar::from(321)))]
                        .into_iter()
                        .collect()
                ))
            )]
            .into_iter()
            .collect()
        }),
        Err(BlockchainError::FullStateNotValid)
    ));
    chain.fork_on_ram().update_states(&ZkBlockchainPatch {
        patches: [(cid, zk::ZkStatePatch::Delta(state_delta.clone()))]
            .into_iter()
            .collect(),
    })?;
    assert!(matches!(
        chain.fork_on_ram().update_states(&ZkBlockchainPatch {
            patches: HashMap::new()
        }),
        Err(BlockchainError::FullStateNotFound)
    ));
    assert!(matches!(
        chain.fork_on_ram().update_states(&ZkBlockchainPatch {
            patches: [(
                cid,
                zk::ZkStatePatch::Full(zk::ZkState {
                    rollbacks: vec![],
                    data: zk::ZkDataPairs(
                        [(zk::ZkDataLocator(vec![100]), zk::ZkScalar::from(200))]
                            .into_iter()
                            .collect()
                    )
                })
            )]
            .into_iter()
            .collect()
        }),
        Err(BlockchainError::FullStateNotValid)
    ));
    chain.fork_on_ram().update_states(&ZkBlockchainPatch {
        patches: [(
            cid,
            zk::ZkStatePatch::Full(zk::ZkState {
                rollbacks: vec![],
                data: zk::ZkDataPairs(
                    [
                        (zk::ZkDataLocator(vec![100]), zk::ZkScalar::from(200)),
                        (zk::ZkDataLocator(vec![123]), zk::ZkScalar::from(234)),
                    ]
                    .into_iter()
                    .collect(),
                ),
            }),
        )]
        .into_iter()
        .collect(),
    })?;
    chain.fork_on_ram().update_states(&ZkBlockchainPatch {
        patches: [(
            cid,
            zk::ZkStatePatch::Full(zk::ZkState {
                rollbacks: vec![],
                data: zk::ZkDataPairs(
                    [
                        (zk::ZkDataLocator(vec![100]), zk::ZkScalar::from(200)),
                        (zk::ZkDataLocator(vec![123]), zk::ZkScalar::from(234)),
                    ]
                    .into_iter()
                    .collect(),
                ),
            }),
        )]
        .into_iter()
        .collect(),
    })?;
    chain.fork_on_ram().update_states(&ZkBlockchainPatch {
        patches: [(cid, zk::ZkStatePatch::Full(full_state_with_delta))]
            .into_iter()
            .collect(),
    })?;
    let mut unupdated_fork = chain.fork_on_ram();
    let mut updated_fork = chain.fork_on_ram();
    updated_fork.update_states(&ZkBlockchainPatch {
        patches: [(
            cid,
            zk::ZkStatePatch::Delta(zk::ZkDeltaPairs(
                [(zk::ZkDataLocator(vec![123]), Some(zk::ZkScalar::from(234)))]
                    .into_iter()
                    .collect(),
            )),
        )]
        .into_iter()
        .collect(),
    })?;
    assert_eq!(updated_fork.get_outdated_contracts()?.len(), 0);
    let updated_tip_hash = updated_fork.get_tip()?.hash();

    let outdated_heights = unupdated_fork.get_outdated_heights()?;
    assert_eq!(outdated_heights.len(), 1);

    let gen_state_patch = updated_fork.generate_state_patch(outdated_heights, updated_tip_hash)?;
    unupdated_fork.update_states(&gen_state_patch)?;
    assert_eq!(unupdated_fork.get_outdated_contracts()?.len(), 0);
    chain.update_states(&draft.patch)?;

    assert_eq!(chain.get_height()?, 2);
    assert_eq!(chain.get_outdated_contracts()?.len(), 0);

    assert!(matches!(
        chain.apply_tx(
            &alice
                .call_function(
                    cid,
                    0,
                    state_delta.clone(),
                    state_model.compress::<ZkHasher>(&full_state.data)?,
                    zk::ZkProof::Dummy(true),
                    Money(0),
                    Money(0),
                    1,
                )
                .tx,
            false
        ),
        Err(BlockchainError::InvalidTransactionNonce)
    ));

    assert!(matches!(
        chain.apply_tx(
            &alice
                .call_function(
                    ContractId::from_str(
                        "0000000000000000000000000000000000000000000000000000000000000000"
                    )
                    .unwrap(),
                    0,
                    state_delta.clone(),
                    state_model.compress::<ZkHasher>(&full_state.data)?,
                    zk::ZkProof::Dummy(true),
                    Money(0),
                    Money(0),
                    2,
                )
                .tx,
            false
        ),
        Err(BlockchainError::ContractNotFound)
    ));

    assert!(matches!(
        chain.apply_tx(
            &alice
                .call_function(
                    cid,
                    1,
                    state_delta.clone(),
                    state_model.compress::<ZkHasher>(&full_state.data)?,
                    zk::ZkProof::Dummy(true),
                    Money(0),
                    Money(0),
                    2,
                )
                .tx,
            false
        ),
        Err(BlockchainError::ContractFunctionNotFound)
    ));

    assert!(matches!(
        chain.apply_tx(
            &alice
                .call_function(
                    cid,
                    0,
                    state_delta,
                    state_model.compress::<ZkHasher>(&full_state.data)?,
                    zk::ZkProof::Dummy(false),
                    Money(0),
                    Money(0),
                    2,
                )
                .tx,
            false
        ),
        Err(BlockchainError::IncorrectZkProof)
    ));

    chain.rollback()?;

    assert_eq!(chain.get_height()?, 1);
    assert_eq!(chain.get_outdated_contracts()?.len(), 0);

    chain.rollback()?;

    assert_eq!(chain.get_height()?, 0);
    assert_eq!(chain.get_outdated_contracts()?.len(), 0);

    Ok(())
}
