use std::sync::Once;

pub struct IsoV8 {
    isolate: v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
}

impl IsoV8 {
    pub fn new() -> Self {
        init_v8();
        let mut isolate = v8::Isolate::new(Default::default());
        let context = initialize_slots(&mut isolate);
        Self { isolate, context }
    }

    pub fn scope<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(&mut v8::ContextScope<v8::HandleScope>) -> T,
    {
        let scope = &mut v8::HandleScope::new(&mut self.isolate);
        let context = v8::Local::new(scope, self.context.clone());
        let scope = &mut v8::ContextScope::new(scope, context);
        func(scope)
    }

    pub fn global<T>(&mut self) -> v8::Global<v8::Object> {
        self.scope(|scope| {
            let global = scope.get_current_context().global(scope);
            v8::Global::new(scope, global)
        })
    }
}

static INIT: Once = Once::new();

fn init_v8() {
    INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

pub struct GlobalContext {
    context: v8::Global<v8::Context>,
}

fn initialize_slots(isolate: &mut v8::Isolate) -> v8::Global<v8::Context> {
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let global_context = v8::Global::new(scope, context);
    scope.set_slot(GlobalContext {
        context: global_context.clone(),
    });
    global_context
}
