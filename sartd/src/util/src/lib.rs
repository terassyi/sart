const TYPE_URL_PREFACE: &str = "type.googleapis.com/sart.v1.";

pub fn to_any<T: prost::Message>(m: T, name: &str) -> prost_types::Any {
    let mut v = Vec::new();
    m.encode(&mut v).unwrap();
    prost_types::Any {
        type_url: format!("{}{}", TYPE_URL_PREFACE, name),
        value: v,
    }
}

pub fn type_url(t: &str) -> String {
    format!("{}{}", TYPE_URL_PREFACE, t)
}
