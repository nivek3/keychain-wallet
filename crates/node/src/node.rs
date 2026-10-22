use super::{find_helper, hex, unlock, validate, Selection};
use napi::{Error, Result};
use napi_derive::napi;
use serde_json::Value;
use std::path::Path;

#[napi(object)]
pub struct JsKeyPair {
    pub public_key: String,
    pub private_key: String,
}

#[napi]
pub async fn get_key_pair(input: Value, root: String, home: String) -> Result<JsKeyPair> {
    // Authentication must not occupy Node's shared libuv worker pool.
    let output = tokio::task::spawn_blocking(move || {
        let (name, curve, path, helper) = validate(&input)?;
        let helper = find_helper(helper, Some(home.into()), Path::new(&root))?;
        unlock(Selection {
            helper,
            name,
            curve,
            path,
        })
    })
    .await
    .map_err(|_| Error::from_reason("BRIDGE_ERROR"))?
    .map_err(Error::from_reason)?;
    Ok(JsKeyPair {
        public_key: output.public,
        private_key: hex(&*output.private),
    })
}
