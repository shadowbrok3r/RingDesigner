//! Browser file delivery: bytes as a Blob handed to a download link.

use wasm_bindgen::{JsCast, JsValue};

/// Offers `bytes` to the browser as a download named `name`.
pub fn download(name: &str, mime: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let err = |e: JsValue| anyhow::anyhow!("{e:?}");
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| anyhow::anyhow!("no document"))?;
    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(err)?
        .dyn_into()
        .map_err(|_| anyhow::anyhow!("not an anchor"))?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    web_sys::Url::revoke_object_url(&url).map_err(err)?;
    Ok(())
}
