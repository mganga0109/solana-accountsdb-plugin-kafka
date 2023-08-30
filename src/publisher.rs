// Copyright 2022 Blockdaemon Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use {
    crate::{
        message_wrapper::EventMessage::{self, Account, Slot, Transaction},
        prom::{
            StatsThreadedProducerContext, UPLOAD_ACCOUNTS_TOTAL, UPLOAD_SLOTS_TOTAL,
            UPLOAD_TRANSACTIONS_TOTAL,
        },
        Config, MessageWrapper, SlotStatusEvent, TransactionEvent, UpdateAccountEvent,
    },
    anyhow::Context,
    prost::Message,
    rdkafka::producer::{BaseRecord, Producer, ThreadedProducer},
    std::time::Duration,
    bs58,
};

pub struct Publisher {
    producer: ThreadedProducer<StatsThreadedProducerContext>,
    shutdown_timeout: Duration,

    update_account_topic: String,
    slot_status_topic: String,
    transaction_topic: String,
    publish_separate_program: bool,

    wrap_messages: bool,
}

impl Publisher {
    pub fn new(producer: ThreadedProducer<StatsThreadedProducerContext>, config: &Config) -> Self {
        Self {
            producer,
            shutdown_timeout: Duration::from_millis(config.shutdown_timeout_ms),
            update_account_topic: config.update_account_topic.clone(),
            slot_status_topic: config.slot_status_topic.clone(),
            transaction_topic: config.transaction_topic.clone(),
            publish_separate_program: config.publish_separate_program.clone(),
        }
    }

    pub fn update_account(&self, ev: UpdateAccountEvent) -> Result<(), KafkaError> {
        let topic_with_suffix;

        if self.publish_separate_program {
            let pubkey_base58 = bs58::encode(&ev.owner).into_string();
            topic_with_suffix = format!("{}-{}", self.update_account_topic, pubkey_base58);
        } else {
            topic_with_suffix = format!("{}", self.update_account_topic);
        }

        let buf = ev.encode_to_vec();
        let record = BaseRecord::<Vec<u8>, _>::to(&topic_with_suffix)
            .key(&ev.pubkey)
            wrap_messages: config.wrap_messages,
        }
    }

    pub fn update_account(&self, ev: UpdateAccountEvent) -> anyhow::Result<()> {
        let temp_key;
        let (key, buf) = if self.wrap_messages {
            temp_key = self.copy_and_prepend(ev.pubkey.as_slice(), 65u8);
            (&temp_key, Self::encode_with_wrapper(Account(Box::new(ev))))
        } else {
            (&ev.pubkey, ev.encode_to_vec())
        };
        let record = BaseRecord::<Vec<u8>, _>::to(&self.update_account_topic)
            .key(key)
            .payload(&buf);
        let result = self.producer.send(record).map(|_| ()).map_err(|(e, _)| e);
        UPLOAD_ACCOUNTS_TOTAL
            .with_label_values(&[if result.is_ok() { "success" } else { "failed" }])
            .inc();
        result.with_context(|| {
            format!(
                "Failed to send account to topic: {}",
                self.update_account_topic
            )
        })
    }

    pub fn update_slot_status(&self, ev: SlotStatusEvent) -> anyhow::Result<()> {
        let temp_key;
        let (key, buf) = if self.wrap_messages {
            temp_key = self.copy_and_prepend(&ev.slot.to_le_bytes(), 83u8);
            (&temp_key, Self::encode_with_wrapper(Slot(Box::new(ev))))
        } else {
            temp_key = ev.slot.to_le_bytes().to_vec();
            (&temp_key, ev.encode_to_vec())
        };
        let record = BaseRecord::<Vec<u8>, _>::to(&self.slot_status_topic)
            .key(key)
            .payload(&buf);
        let result = self.producer.send(record).map(|_| ()).map_err(|(e, _)| e);
        UPLOAD_SLOTS_TOTAL
            .with_label_values(&[if result.is_ok() { "success" } else { "failed" }])
            .inc();
        result.with_context(|| {
            format!(
                "Failed to send slot status to topic: {}",
                self.slot_status_topic
            )
        })
    }

    pub fn update_transaction(&self, ev: TransactionEvent) -> anyhow::Result<()> {
        let temp_key;
        let (key, buf) = if self.wrap_messages {
            temp_key = self.copy_and_prepend(ev.signature.as_slice(), 84u8);
            (
                &temp_key,
                Self::encode_with_wrapper(Transaction(Box::new(ev))),
            )
        } else {
            (&ev.signature, ev.encode_to_vec())
        };
        let record = BaseRecord::<Vec<u8>, _>::to(&self.transaction_topic)
            .key(key)
            .payload(&buf);
        let result = self.producer.send(record).map(|_| ()).map_err(|(e, _)| e);
        UPLOAD_TRANSACTIONS_TOTAL
            .with_label_values(&[if result.is_ok() { "success" } else { "failed" }])
            .inc();
        result.with_context(|| {
            format!(
                "Failed to send transaction to topic: {}",
                self.transaction_topic
            )
        })
    }

    pub fn wants_update_account(&self) -> bool {
        !self.update_account_topic.is_empty()
    }

    pub fn wants_slot_status(&self) -> bool {
        !self.slot_status_topic.is_empty()
    }

    pub fn wants_transaction(&self) -> bool {
        !self.transaction_topic.is_empty()
    }

    fn encode_with_wrapper(message: EventMessage) -> Vec<u8> {
        MessageWrapper {
            event_message: Some(message),
        }
        .encode_to_vec()
    }

    fn copy_and_prepend(&self, data: &[u8], prefix: u8) -> Vec<u8> {
        let mut temp_key = Vec::with_capacity(data.len() + 1);
        temp_key.push(prefix);
        temp_key.extend_from_slice(data);
        temp_key
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        let _ = self.producer.flush(self.shutdown_timeout);
    }
}
