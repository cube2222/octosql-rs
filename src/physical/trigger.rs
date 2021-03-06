// Copyright 2020 The OctoSQL Authors
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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Builder, StringBuilder};
use arrow::datatypes::DataType;

use crate::physical::arrow::{create_key, GroupByScalar};

pub trait TriggerPrototype: Send + Sync {
    fn create_trigger(&self, key_data_types: Vec<DataType>) -> Box<dyn Trigger>;
}

#[derive(Debug)]
pub struct CountingTriggerPrototype {
    pub trigger_count: u64
}

impl CountingTriggerPrototype {
    pub fn new(trigger_count: u64) -> CountingTriggerPrototype {
        CountingTriggerPrototype {
            trigger_count,
        }
    }
}

impl TriggerPrototype for CountingTriggerPrototype {
    fn create_trigger(&self, key_data_types: Vec<DataType>) -> Box<dyn Trigger> {
        Box::new(CountingTrigger::new(key_data_types, self.trigger_count))
    }
}

pub trait Trigger: std::fmt::Debug {
    fn keys_received(&mut self, keys: Vec<ArrayRef>);
    fn poll(&mut self) -> Vec<ArrayRef>;
}

#[derive(Debug)]
pub struct CountingTrigger {
    key_data_types: Vec<DataType>,
    trigger_count: u64,
    counts: BTreeMap<Vec<GroupByScalar>, u64>,
    to_trigger: BTreeSet<Vec<GroupByScalar>>,
}

impl CountingTrigger {
    pub fn new(key_data_types: Vec<DataType>, trigger_count: u64) -> CountingTrigger {
        CountingTrigger {
            key_data_types,
            trigger_count,
            counts: Default::default(),
            to_trigger: Default::default(),
        }
    }
}

impl Trigger for CountingTrigger {
    fn keys_received(&mut self, keys: Vec<ArrayRef>) {
        let mut key_vec: Vec<GroupByScalar> = Vec::with_capacity(keys.len());
        for _i in 0..self.key_data_types.len() {
            key_vec.push(GroupByScalar::Int64(0))
        }

        for row in 0..keys[0].len() {
            create_key(keys.as_slice(), row, &mut key_vec).unwrap();

            let count = self.counts.entry(key_vec.clone()).or_insert(0);
            *count += 1;
            if *count == self.trigger_count {
                *count = 0; // TODO: Delete
                self.to_trigger.insert(key_vec.clone());
            }
        }
    }

    fn poll(&mut self) -> Vec<ArrayRef> {
        let mut output_columns: Vec<ArrayRef> = Vec::with_capacity(self.key_data_types.len());
        for key_index in 0..self.key_data_types.len() {
            match self.key_data_types[key_index] {
                DataType::Utf8 => {
                    let mut array = StringBuilder::new(self.to_trigger.len());
                    self.to_trigger.iter().for_each(|k| {
                        match &k[key_index] {
                            GroupByScalar::Utf8(text) => array.append_value(text.as_str()).unwrap(),
                            _ => panic!("bug: key doesn't match schema"),
                            // TODO: Maybe use as_any -> downcast?
                        }
                    });
                    output_columns.push(Arc::new(array.finish()) as ArrayRef);
                }
                DataType::Int64 => {
                    let mut array = Int64Builder::new(self.to_trigger.len());
                    self.to_trigger.iter().for_each(|k| {
                        match k[key_index] {
                            GroupByScalar::Int64(n) => array.append_value(n).unwrap(),
                            _ => panic!("bug: key doesn't match schema"),
                            // TODO: Maybe use as_any -> downcast?
                        }
                    });
                    output_columns.push(Arc::new(array.finish()) as ArrayRef);
                }
                _ => unimplemented!(),
            }
        }
        self.to_trigger.clear();
        output_columns
    }
}
