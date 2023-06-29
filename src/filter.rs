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
    crate::*,
    solana_program::pubkey::Pubkey,
    std::{collections::HashSet, str::FromStr},
    config::{ConfigFiltersAccounts, ConfigFiltersMemcmp},
    bs58,
};

pub struct FiltersAccounts {
    pub program_id: Option<[u8; 32]>,
    pub data_size: Option<usize>,
    pub memcmp: Option<FiltersMemcmp>
}

pub struct FiltersMemcmp {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

pub struct Filter {
    program_ignores: HashSet<[u8; 32]>,
    program_filters: HashSet<[u8; 32]>,
    account_filters: HashSet<[u8; 32]>,
    filters: Vec<FiltersAccounts>,
}

impl Filter {
    pub fn new(config: &Config) -> Self {
        Self {
            program_ignores: config
                .program_ignores
                .iter()
                .flat_map(|p| Pubkey::from_str(p).ok().map(|p| p.to_bytes()))
                .collect(),
            program_filters: config
                .program_filters
                .iter()
                .flat_map(|p| Pubkey::from_str(p).ok().map(|p| p.to_bytes()))
                .collect(),
            account_filters: config
                .account_filters
                .iter()
                .flat_map(|p| Pubkey::from_str(p).ok().map(|p| p.to_bytes()))
                .collect(),
            filters: config
                .filters
                .iter()
                .map(|filter| {
                    let program_id = Pubkey::from_str(&filter.program_id)
                        .ok()
                        .map(|program_id| program_id.to_bytes());

                        let memcmp = match &filter.memcmp {
                            Some(memcmp) => FiltersMemcmp {
                                offset: memcmp.offset,
                                bytes: match bs58::decode(&memcmp.bytes).into_vec() {
                                    Ok(decoded_bytes) => decoded_bytes,
                                    Err(_) => {
                                        // Handle the error case appropriately
                                        // For example, provide a default value or panic
                                        panic!("Failed to decode bs58-encoded bytes");
                                    }
                                },
                            },
                            None => FiltersMemcmp {
                                offset: 0,  // Provide a default value here
                                bytes: Vec::new(),  // Provide a default value here
                            },
                        };

                    FiltersAccounts {
                        program_id,
                        data_size: filter.data_size,
                        memcmp: Some(memcmp),
                    }
                })
                .collect(),
        }
    }

    pub fn wants_program(&self, program: &[u8]) -> bool {
        let key = match <&[u8; 32]>::try_from(program) {
            Ok(key) => key,
            _ => return true,
        };
        !self.program_ignores.contains(key)
            && (self.program_filters.is_empty() || self.program_filters.contains(key))
    }

    pub fn wants_filter(&self, program: &[u8], pubkey: &[u8], data: &[u8], lamports: u64) -> bool {
        let program_input = match <&[u8; 32]>::try_from(program) {
            Ok(program_input) => program_input,
            _ => return true,
        };

        let mut res = false;
        for filter in &self.filters {
            // Access individual filter properties
            let program_id = &filter.program_id;
            let data_size = filter.data_size;

            match program_id {
                Some(id) => {
                    if program_input != id {
                        continue;
                    }
                }
                None => {
                    // Handle the case when program_id is None
                }
            }

            match data_size {
                Some(size) => {
                    if data.len() != size {
                        continue;
                    }
                }
                None => {
                    // Handle the case when program_id is None
                }
            }

            if let Some(memcmp) = &filter.memcmp {
                if memcmp.bytes.len() == 0 {
                    continue;
                }

                if memcmp.offset + memcmp.bytes.len() > data.len() {
                    continue;
                }
                if memcmp.bytes != &data[memcmp.offset..memcmp.offset + memcmp.bytes.len()] {
                    continue;
                }
            }

            res = true;
        }

        res
        // !self.program_ignores.contains(key)
        //     && (self.program_filters.is_empty() || self.program_filters.contains(key))

    }

    pub fn wants_account(&self, account: &[u8]) -> bool {
        let key = match <&[u8; 32]>::try_from(account) {
            Ok(key) => key,
            _ => return true,
        };
        self.account_filters.contains(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_v2() {
        let config = Config {
            filters: vec![
                ConfigFiltersAccounts {
                    program_id: "Sysvar1111111111111111111111111111111111111".to_owned(),
                    data_size: Some(32),
                    // memcmp: None
                    memcmp: Some(ConfigFiltersMemcmp {
                        offset: 0,
                        bytes: (&"9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin").to_string()
                    }),
                },
                // ConfigFiltersAccounts {
                //     program_id: "Sysvar1111111111111111111111111111111111111".to_owned(),
                //     data_size: 32,
                //     memcmp: ConfigFiltersMemcmp {
                //         offset: 0,
                //         bytes: (&"abcd").to_string()
                //     },
                // },
            ],
            program_filters: vec!["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_owned()],
            ..Config::default()
        };

        println!("{:?}", config.filters);

        let filter = Filter::new(&config);

        assert!(filter.wants_filter(
            &Pubkey::from_str("Sysvar1111111111111111111111111111111111111")
                .unwrap()
                .to_bytes(),
            &Pubkey::from_str("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin")
                .unwrap()
                .to_bytes(),
            &Pubkey::from_str("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin")
                .unwrap()
                .to_bytes(),
            "Sysvar1111111111111111111111111111111111111".to_owned()
        ));

    }

    #[test]
    fn test_filter() {
        let config = Config {
            program_ignores: vec![
                "Sysvar1111111111111111111111111111111111111".to_owned(),
                "Vote111111111111111111111111111111111111111".to_owned(),
            ],
            program_filters: vec!["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_owned()],
            ..Config::default()
        };

        let filter = Filter::new(&config);
        assert_eq!(filter.program_ignores.len(), 2);

        assert!(filter.wants_program(
            &Pubkey::from_str("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin")
                .unwrap()
                .to_bytes()
        ));
        assert!(!filter.wants_program(
            &Pubkey::from_str("Vote111111111111111111111111111111111111111")
                .unwrap()
                .to_bytes()
        ));
    }

    #[test]
    fn test_owner_filter() {
        let config = Config {
            program_ignores: vec![
                "Sysvar1111111111111111111111111111111111111".to_owned(),
                "Vote111111111111111111111111111111111111111".to_owned(),
            ],
            program_filters: vec!["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_owned()],
            ..Config::default()
        };

        let filter = Filter::new(&config);
        assert_eq!(filter.program_ignores.len(), 2);

        assert!(filter.wants_program(
            &Pubkey::from_str("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin")
                .unwrap()
                .to_bytes()
        ));
        assert!(!filter.wants_program(
            &Pubkey::from_str("Vote111111111111111111111111111111111111111")
                .unwrap()
                .to_bytes()
        ));

        assert!(!filter.wants_program(
            &Pubkey::from_str("cndy3Z4yapfJBmL3ShUp5exZKqR3z33thTzeNMm2gRZ")
                .unwrap()
                .to_bytes()
        ));
    }

    #[test]
    fn test_account_filter() {
        let config = Config {
            program_filters: vec!["9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".to_owned()],
            account_filters: vec!["5KKsLVU6TcbVDK4BS6K1DGDxnh4Q9xjYJ8XaDCG5t8ht".to_owned()],
            ..Config::default()
        };

        let filter = Filter::new(&config);
        assert_eq!(filter.program_filters.len(), 1);
        assert_eq!(filter.account_filters.len(), 1);

        println!("{:?}", filter.account_filters);
        println!(
            "{:?}",
            &Pubkey::from_str("5KKsLVU6TcbVDK4BS6K1DGDxnh4Q9xjYJ8XaDCG5t8ht")
                .unwrap()
                .to_bytes()
        );

        assert!(filter.wants_program(
            &Pubkey::from_str("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin")
                .unwrap()
                .to_bytes()
        ));

        assert!(filter.wants_account(
            &Pubkey::from_str("5KKsLVU6TcbVDK4BS6K1DGDxnh4Q9xjYJ8XaDCG5t8ht")
                .unwrap()
                .to_bytes()
        ));
    }
}
