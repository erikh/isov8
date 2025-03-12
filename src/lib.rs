pub mod value;

use crate::value::*;
use std::sync::{Arc, Mutex, Once};

pub type Result = std::result::Result<Value, Error>;

lazy_static::lazy_static! {
    static ref ISOLATE: Arc<Mutex<Option<v8::OwnedIsolate>>> = Arc::new(Mutex::new(None));
}

#[derive(Debug)]
pub enum Error {
    Timeout,
    Value(String),
}

pub struct IsoV8 {
    context: v8::Global<v8::Context>,
}

impl IsoV8 {
    pub fn new() -> Self {
        init_v8();
        let mut lock = ISOLATE.lock().unwrap();
        if let None = *lock {
            let isolate = v8::Isolate::new(Default::default());
            ISOLATE.lock().unwrap().replace(isolate);
        };

        let mut isolate = lock.take().unwrap();
        let context = initialize_slots(&mut isolate);
        lock.replace(isolate);
        Self { context }
    }

    pub fn eval(&mut self, source: impl Into<String>) -> Result {
        self.try_catch(|scope| {
            let source = create_string(scope, source);
            let script = v8::Script::compile(scope, source, None);
            exception(scope)?;
            let result = script.unwrap().run(scope).unwrap();
            exception(scope)?;
            Ok(Value::new(scope, result))
        })
    }

    pub fn try_catch<F>(&mut self, func: F) -> Result
    where
        F: FnOnce(&mut v8::TryCatch<v8::HandleScope>) -> Result,
    {
        self.scope(|scope| func(&mut v8::TryCatch::new(scope)))
    }

    pub fn scope<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(&mut v8::ContextScope<v8::HandleScope>) -> T,
    {
        let mut lock = ISOLATE.lock().unwrap();
        let isolate = lock.as_mut().unwrap();
        let scope = &mut v8::HandleScope::new(isolate);
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

fn initialize_slots(isolate: &mut v8::Isolate) -> v8::Global<v8::Context> {
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let global_context = v8::Global::new(scope, context);
    global_context
}

pub fn exception(scope: &mut v8::TryCatch<v8::HandleScope>) -> Result {
    if scope.has_terminated() {
        Err(Error::Timeout)
    } else if let Some(exception) = scope.exception() {
        Err(Error::Value(exception.to_rust_string_lossy(scope)))
    } else {
        Ok(Value::NoValue)
    }
}

fn create_string<'s>(
    scope: &mut v8::HandleScope<'s>,
    value: impl Into<String>,
) -> v8::Local<'s, v8::String> {
    v8::String::new(scope, value.into().as_str()).expect("string exceeds maximum length")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_basic_eval() {
        let mut iso = IsoV8::new();
        let result = iso.eval("1 + 1").unwrap();
        assert_eq!(result, Value::Float(2.0));
        let result = iso.eval("[1, 2]").unwrap();
        let a = Value::Array(vec![Value::Float(1.0), Value::Float(2.0)]);
        assert_eq!(result, a);
        let result = iso.eval("({ \"test\": 1 })").unwrap();
        let mut map = BTreeMap::default();
        map.insert(Value::String("test".to_string()), Value::Float(1.0));
        let o = Value::Object(map);
        assert_eq!(result, o);
        let result = iso
            .eval(
                r#"
            function foo() {
                return 1
            }

            foo();
            "#,
            )
            .unwrap();
        assert_eq!(result, Value::Float(1.0));
    }

    #[test]
    fn test_types() {
        let mut map = BTreeMap::default();
        map.insert(Value::String("test".to_string()), Value::Float(1.0));
        let o = Value::Object(map);
        let mut iso = IsoV8::new();
        let table = vec![
            ("1 + 1", Value::Float(2.0), "numbers"),
            (
                "[1, 2]",
                Value::Array(vec![Value::Float(1.0), Value::Float(2.0)]),
                "array of numbers",
            ),
            ("({ \"test\": 1 })", o, "object"),
            ("undefined", Value::Undefined, "undefined"),
            ("null", Value::Null, "null"),
            (
                "new Date(\"2025-03-11\")",
                Value::Date(1741651200000.0),
                "date",
            ),
            (
                r#"'this is a string'"#,
                Value::String("this is a string".to_string()),
                "string",
            ),
        ];

        for item in table {
            eprintln!("{}", item.2);
            let result = iso.eval(item.0).unwrap();
            assert_eq!(result, item.1, "{}", item.2);
        }
    }
}
