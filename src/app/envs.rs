use anyhow::Result;
use std::collections::HashMap;

use super::*;
use crate::storage::{EnvMeta, StoredEnv};

impl App {
    pub(super) fn create_env(&mut self, name: String) -> Result<()> {
        let env = StoredEnv {
            env: EnvMeta { name },
            vars: HashMap::new(),
        };
        crate::storage::save_env(&env)?;
        self.environments.push(env);
        self.env_cursor = self.environments.len() - 1;
        self.env_var_cursor = 0;
        Ok(())
    }

    pub(super) fn add_var(&mut self, key: String, value: String, env_idx: usize) -> Result<()> {
        self.environments[env_idx].vars.insert(key, value);
        crate::storage::save_env(&self.environments[env_idx])?;
        Ok(())
    }

    pub(super) fn open_env_delete_modal(&mut self) {
        match self.env_focus {
            EnvFocus::Envs => {
                if let Some(env) = self.environments.get(self.env_cursor) {
                    self.modal = Some(ModalState::ConfirmDelete {
                        label: env.env.name.clone(),
                        address: NodeAddress::Env(self.env_cursor),
                    });
                }
            }
            EnvFocus::Vars => {
                if let Some(env) = self.environments.get(self.env_cursor) {
                    let vars = sorted_vars(env);
                    if let Some((key, _)) = vars.get(self.env_var_cursor) {
                        self.modal = Some(ModalState::ConfirmDelete {
                            label: key.clone(),
                            address: NodeAddress::EnvVar {
                                env_idx: self.env_cursor,
                                key: key.clone(),
                            },
                        });
                    }
                }
            }
        }
    }
}
