use near_sdk::serde_json::Value;

pub trait MetaData {
    /// returns None if the contract does not support the requested metadata
    fn metadata(uri: String) -> Option<Value>;

    /// returns the metadata that this contract exposes
    fn metadata_uris() -> Vec<String>;
}
