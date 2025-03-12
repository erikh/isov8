use std::collections::BTreeMap;

pub type Array = Vec<Value>;
pub type Object = BTreeMap<Value, Value>;

#[derive(Debug)]
pub enum Value {
    NoValue,
    Undefined,
    Null,
    Boolean(bool),
    Float(f64),
    Integer(i32),
    UnsignedInteger(u32),
    Date(f64),
    String(String),
    Array(Array),
    Function(v8::Function),
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
            Self::NoValue => matches!(other, Self::NoValue),
            Self::Null => matches!(other, Self::Null),
            Self::Undefined => matches!(other, Self::Undefined),
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
