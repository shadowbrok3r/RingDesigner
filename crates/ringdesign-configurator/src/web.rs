//! Browser file delivery: bytes as a Blob handed to a download link.

use wasm_bindgen::{JsCast, JsValue};

pub fn pick_project(tx: std::sync::mpsc::Sender<Result<String, String>>, ctx: egui::Context) -> anyhow::Result<()> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| anyhow::anyhow!("No document"))?;
    let input: web_sys::HtmlInputElement = document.create_element("input").map_err(|e| anyhow::anyhow!("{e:?}"))?.unchecked_into();
    input.set_type("file"); input.set_accept(".json,application/json");
    let read = input.clone();
    let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
        if let Some(file) = read.files().and_then(|f| f.get(0)) {
            wasm_bindgen_futures::spawn_local(async move {
                let result = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                    .map_err(|e| format!("Cannot read project: {e:?}"))
                    .and_then(|buffer| String::from_utf8(js_sys::Uint8Array::new(&buffer).to_vec()).map_err(|e| e.to_string()));
                let _ = tx.send(result); ctx.request_repaint();
            });
        }
        read.set_onchange(None); read.set_oncancel(None); read.remove();
    });
    input.set_onchange(Some(callback.unchecked_ref()));
    input.set_oncancel(Some(callback.unchecked_ref()));
    input.set_attribute("style", "display:none").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    document.body().ok_or_else(|| anyhow::anyhow!("No document body"))?.append_child(&input).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    input.click();
    Ok(())
}

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
