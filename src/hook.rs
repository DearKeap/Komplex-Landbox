use std::collections::HashMap;

/// Context generik yang dikirim ke tiap hook callback.
/// Nanti ini yang jadi jembatan ke Lua/Luau value pas ModLoad masuk —
/// buat sekarang cukup key-value string sederhana.
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    pub data: HashMap<String, String>,
}

impl HookContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.data.insert(key.to_string(), value.into());
        self
    }
}

type HookCallback = Box<dyn Fn(&HookContext)>;

/// Registry generik — PreLoader gak peduli hook APA yang exist,
/// cuma nyediain plumbing register/fire. Ini yang bikin dia "hook-agnostic"
/// sesuai desain (PreLoader ringan, hook konkret ditambahin ModLoad).
#[derive(Default)]
pub struct HookRegistry {
    hooks: HashMap<String, Vec<HookCallback>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// name pakai format ":event" (Astatin) atau "MetaMod.id:event" (community).
    /// Validasi format lengkap (parsing origin/path) belum diimplementasi di
    /// skeleton ini — baru primitif dasarnya.
    pub fn register(&mut self, name: &str, callback: HookCallback) {
        self.hooks.entry(name.to_string()).or_default().push(callback);
    }

    pub fn fire(&self, name: &str, ctx: &HookContext) {
        if let Some(callbacks) = self.hooks.get(name) {
            for cb in callbacks {
                cb(ctx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_fire() {
        let mut registry = HookRegistry::new();
        registry.register(":entity.death", Box::new(|ctx| {
            assert_eq!(ctx.data.get("entity_id").map(|s| s.as_str()), Some("42"));
        }));
        registry.fire(":entity.death", &HookContext::new().with("entity_id", "42"));
    }
}
