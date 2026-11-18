//! Built-in loader adapter registry. Third-party providers can be registered
//! by hosts without changing the stable adapter contracts.
use std::{collections::HashMap, sync::Arc};
use umc_loader_api::{LoaderAdapter, LoaderId};

#[derive(Clone)]
pub struct LoaderRegistry {
    adapters: HashMap<LoaderId, Arc<dyn LoaderAdapter>>,
}

impl Default for LoaderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LoaderRegistry {
    pub fn new() -> Self {
        let adapters: Vec<Arc<dyn LoaderAdapter>> = vec![
            Arc::new(umc_loader_fabric::FabricAdapter::new()),
            Arc::new(umc_loader_forge::ForgeAdapter::new()),
            Arc::new(umc_loader_neoforge::NeoForgeAdapter::new()),
        ];
        let mut registry = Self {
            adapters: HashMap::new(),
        };
        for adapter in adapters {
            // Built-in ids are unique by construction.
            registry.adapters.insert(adapter.id(), adapter);
        }
        registry
    }

    /// Register or replace an adapter. Hosts decide which third-party providers
    /// to trust and expose; the registry performs no dynamic code loading.
    pub fn register(&mut self, adapter: Arc<dyn LoaderAdapter>) {
        self.adapters.insert(adapter.id(), adapter);
    }

    pub fn get(&self, id: &LoaderId) -> Option<Arc<dyn LoaderAdapter>> {
        self.adapters.get(id).cloned()
    }

    pub fn available(&self) -> Vec<LoaderId> {
        let mut ids: Vec<_> = self.adapters.keys().cloned().collect();
        ids.sort();
        ids
    }
}
