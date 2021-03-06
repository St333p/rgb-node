// RGB standard library
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use chrono::Utc;
use core::convert::TryFrom;
use serde::Deserialize;
use std::collections::BTreeMap;

use lnpbp::bitcoin::OutPoint;
use lnpbp::bp;
use lnpbp::rgb::prelude::*;
use lnpbp::secp256k1zkp;

use super::schema::{self, FieldType, OwnedRightsType, TransitionType};
use super::{AccountingAmount, Allocation, Asset, Outcoincealed, Outcoins};

use crate::error::{BootstrapError, ServiceErrorDomain};
use crate::util::SealSpec;
use crate::{field, type_map};

pub struct Processor {}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize,),
    serde(crate = "serde_crate")
)]
pub enum IssueStructure {
    SingleIssue,
    MultipleIssues {
        max_supply: f32,
        reissue_control: SealSpec,
    },
}

impl Processor {
    pub fn new() -> Result<Self, BootstrapError> {
        debug!("Instantiating RGB asset manager ...");

        let me = Self {};
        /*
        let storage = rgb_storage.clone();
        let me = Self {
            rgb_storage,
            asset_storage,
        };
         */
        let _schema = schema::schema();
        //if !me.rgb_storage.lock()?.has_schema(schema.schema_id())? {
        info!("RGB fungible assets schema file not found, creating one");
        //storage.lock()?.add_schema(&schema)?;
        //}

        Ok(me)
    }

    pub fn issue(
        &mut self,
        network: bp::Chain,
        ticker: String,
        name: String,
        description: Option<String>,
        issue_structure: IssueStructure,
        allocations: Vec<Outcoins>,
        precision: u8,
        prune_seals: Vec<SealSpec>,
    ) -> Result<(Asset, Genesis), ServiceErrorDomain> {
        let now = Utc::now().timestamp();
        let mut metadata = type_map! {
            FieldType::Ticker => field!(String, ticker),
            FieldType::Name => field!(String, name),
            FieldType::Precision => field!(U8, precision),
            FieldType::Timestamp => field!(I64, now)
        };
        if let Some(description) = description {
            metadata
                .insert(*FieldType::ContractText, field!(String, description));
        }

        let mut issued_supply = 0u64;
        let allocations = allocations
            .into_iter()
            .map(|outcoins| {
                let amount =
                    AccountingAmount::transmutate(precision, outcoins.coins);
                issued_supply += amount;
                (outcoins.seal_definition(), amount)
            })
            .collect();
        let mut owned_rights = BTreeMap::new();
        owned_rights.insert(
            *OwnedRightsType::Assets,
            Assignments::zero_balanced(
                vec![value::Revealed {
                    value: issued_supply,
                    blinding: secp256k1zkp::key::ONE_KEY,
                }],
                allocations,
                vec![],
            ),
        );
        metadata.insert(*FieldType::IssuedSupply, field!(U64, issued_supply));

        if let IssueStructure::MultipleIssues {
            max_supply,
            reissue_control,
        } = issue_structure
        {
            let total_supply =
                AccountingAmount::transmutate(precision, max_supply);
            if total_supply < issued_supply {
                Err(ServiceErrorDomain::Schema(format!(
                    "Total supply ({}) should be greater than the issued supply ({})",
                    total_supply, issued_supply
                )))?;
            }
            owned_rights.insert(
                *OwnedRightsType::Inflation,
                Assignments::Declarative(vec![OwnedState::Revealed {
                    seal_definition: reissue_control.seal_definition(),
                    assigned_state: data::Void,
                }]),
            );
        }

        if prune_seals.len() > 0 {
            owned_rights.insert(
                *OwnedRightsType::BurnReplace,
                Assignments::Declarative(
                    prune_seals
                        .into_iter()
                        .map(|seal_spec| OwnedState::Revealed {
                            seal_definition: seal_spec.seal_definition(),
                            assigned_state: data::Void,
                        })
                        .collect(),
                ),
            );
        }

        let genesis = Genesis::with(
            schema::schema().schema_id(),
            network,
            metadata.into(),
            owned_rights,
            bset![],
            vec![],
        );

        let asset = Asset::try_from(genesis.clone())?;
        //self.asset_storage.lock()?.add_asset(asset.clone())?;

        Ok((asset, genesis))
    }

    /// Function creates a fungible asset-specific state transition (i.e. RGB-20
    /// schema-based) given an asset information, inputs and desired outputs
    pub fn transfer(
        &mut self,
        asset: &mut Asset,
        inputs: Vec<OutPoint>,
        ours: Vec<Outcoins>,
        theirs: Vec<Outcoincealed>,
    ) -> Result<Transition, ServiceErrorDomain> {
        // Collecting all input allocations
        let mut input_allocations = Vec::<Allocation>::new();
        for seal in &inputs {
            let found = asset
                .allocations(seal)
                .ok_or(format!("Unknown input {}", seal))?
                .clone();
            if found.len() == 0 {
                Err(format!("Unknown input {}", seal))?
            }
            input_allocations.extend(found);
        }
        // Computing sum of inputs
        let total_inputs = input_allocations
            .iter()
            .fold(0u64, |acc, alloc| acc + alloc.value().value);

        let metadata = type_map! {};
        let mut total_outputs = 0;
        let allocations_ours = ours
            .into_iter()
            .map(|outcoins| {
                let amount = AccountingAmount::transmutate(
                    *asset.fractional_bits(),
                    outcoins.coins,
                );
                total_outputs += amount;
                (outcoins.seal_definition(), amount)
            })
            .collect();
        let allocations_theirs = theirs
            .into_iter()
            .map(|outcoincealed| {
                let amount = AccountingAmount::transmutate(
                    *asset.fractional_bits(),
                    outcoincealed.coins,
                );
                total_outputs += amount;
                (outcoincealed.seal_confidential, amount)
            })
            .collect();

        if total_inputs != total_outputs {
            Err("Input amount is not equal to output amount".to_string())?
        }

        let input_amounts = input_allocations
            .iter()
            .map(|alloc| alloc.value().clone())
            .collect();
        let assignments = type_map! {
            OwnedRightsType::Assets =>
            Assignments::zero_balanced(input_amounts, allocations_ours, allocations_theirs)
        };

        let mut parent = ParentOwnedRights::new();
        for alloc in input_allocations {
            parent
                .entry(*alloc.node_id())
                .or_insert(bmap! {})
                .entry(*OwnedRightsType::Assets)
                .or_insert(vec![])
                .push(*alloc.index());
        }

        let transition = Transition::with(
            *TransitionType::Transfer,
            metadata.into(),
            parent,
            assignments,
            bset![],
            vec![],
        );

        Ok(transition)
    }
}
