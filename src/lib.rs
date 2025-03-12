use std::collections::BTreeMap;
use std::sync::Once;

pub type Result = std::result::Result<Value, Error>;

pub type Array = Vec<Value>;
pub type Object = BTreeMap<Value, Value>;

#[derive(Debug)]
pub enum Value {
    NoValue,
    Undefined,
    Null,
    /// The JavaScript value `true` or `false`.
    Boolean(bool),
    /// A JavaScript floating point number.
    Float(f64),
    Integer(i32),
    UnsignedInteger(u32),
    /// Elapsed milliseconds since Unix epoch.
    Date(f64),
    /// An immutable JavaScript string, managed by V8.
    String(String),
    /// Reference to a JavaScript array.
    Array(Array),
    /// Reference to a JavaScript function.
    Function(v8::Function),
    /// Reference to a JavaScript object. If a value is a function or an array in JavaScript, it
    /// will be converted to `Value::Array` or `Value::Function` instead of `Value::Object`.
    Object(Object),
}

impl Eq for Value {}

impl Ord for Value {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Boolean(b) => match other {
                Self::Boolean(b2) => b == b2,
                _ => false,
            },
            Self::Float(f) => match other {
                Self::Float(f2) => f == f2,
                _ => false,
            },
            Self::Integer(i) => match other {
                Self::Integer(i2) => i == i2,
                _ => false,
            },
            Self::UnsignedInteger(u) => match other {
                Self::UnsignedInteger(u2) => u == u2,
                _ => false,
            },
            Self::Date(d) => match other {
                Self::Date(d2) => d == d2,
                _ => false,
            },
            Self::String(s) => match other {
                Self::String(s2) => s == s2,
                _ => false,
            },
            Self::Function(f) => match other {
                Self::Function(f2) => f == f2,
                _ => false,
            },
            Self::Object(o) => match other {
                Self::Object(o2) => {
                    for x in o.keys() {
                        if !o2.contains_key(x) {
                            return false;
                        }
                    }
                    for x in o2.keys() {
                        if !o.contains_key(x) {
                            return false;
                        }
                    }

                    true
                }
                _ => false,
            },
            Self::Array(a) => match other {
                Self::Array(a2) => {
                    for x in 0..a.len() {
                        if a2[x] != a[x] {
                            return false;
                        }
                    }

                    true
                }
                _ => false,
            },
            _ => *self == *other,
        }
    }
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::NoValue => state.write_u8(0),
            Self::Undefined => state.write_u8(1),
            Self::Null => state.write_u8(3),
            Self::Boolean(t) => state.write_u8(if *t { 4 } else { 5 }),
            Self::Float(f) => {
                state.write_u8(6);
                state.write_u64(*f as u64)
            }
            Self::Integer(i) => {
                state.write_u8(7);
                state.write_i32(*i)
            }
            Self::UnsignedInteger(u) => {
                state.write_u8(8);
                state.write_u32(*u)
            }
            Self::Date(d) => {
                state.write_u8(9);
                state.write_u64(*d as u64)
            }
            Self::String(s) => {
                state.write_u8(10);
                state.write(s.as_bytes());
            }
            Self::Array(a) => {
                state.write_u8(11);
                for x in a {
                    x.hash(state);
                }
            }
            // FIXME more detail
            Self::Function(_) => {
                state.write_u8(12);
            }
            Self::Object(o) => {
                state.write_u8(13);
                for x in o.keys() {
                    x.hash(state);
                    o.get(x).unwrap().hash(state);
                }
            }
        }
    }
}

impl Value {
    pub fn new(scope: &mut v8::HandleScope<'_>, value: v8::Local<'_, v8::Value>) -> Self {
        if value.is_null() {
            return Self::Null;
        } else if value.is_undefined() {
            return Self::Undefined;
        } else if value.is_number() {
            return Self::Float(value.number_value(scope).unwrap());
        } else if value.is_uint32() {
            return Self::UnsignedInteger(value.uint32_value(scope).unwrap());
        } else if value.is_int32() {
            return Self::Integer(value.int32_value(scope).unwrap());
        } else if value.is_string() {
            return Self::String(value.to_rust_string_lossy(scope));
        } else if value.is_date() {
            return Self::Date(value.number_value(scope).unwrap());
        } else if value.is_boolean() {
            return Self::Boolean(value.boolean_value(scope));
        } else if value.is_object() {
            let obj = value.to_object(scope);
            match obj {
                Some(o) => {
                    if value.is_array() {
                        let ary = value.cast::<v8::Array>();
                        if ary.length() > 0 {
                            let mut new = Array::new();

                            for x in 0..ary.length() {
                                let v = ary.get_index(scope, x).unwrap();
                                new.push(Self::new(scope, v));
                            }

                            return Self::Array(new);
                        }
                    }

                    let ary = o.get_property_names(scope, Default::default()).unwrap();
                    let mut new = Object::default();

                    for x in 0..ary.length() {
                        let k = ary.get_index(scope, x).unwrap();
                        let v = o.get(scope, k).unwrap();
                        new.insert(Self::new(scope, k), Self::new(scope, v));
                    }

                    return Self::Object(new);
                }
                None => Self::NoValue,
            }
        } else {
            Self::NoValue
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Timeout,
    Value(String),
}

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
    }
}
