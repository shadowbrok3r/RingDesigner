#[cfg(not(target_arch = "wasm32"))]
fn main() {}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::{JsCast, closure::Closure};
    let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let out = scope.clone();
    let callback = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
        let Some(text) = event.data().as_string() else {
            return;
        };
        let result = serde_json::from_str::<ringdesign_workbench::job::Job>(&text)
            .map(ringdesign_workbench::job::process);
        if let Ok(done) = result {
            if let Ok(ringdesign_workbench::job::Output::Artifact(file)) = &done.result {
                let object = js_sys::Object::new();
                for (name, value) in [
                    ("id", done.id.to_string()),
                    ("key", done.key.to_string()),
                    ("name", file.name.clone()),
                    ("mime", file.mime.clone()),
                ] {
                    let _ = js_sys::Reflect::set(&object, &name.into(), &value.into());
                }
                let buffer = js_sys::Uint8Array::from(file.bytes.as_slice()).buffer();
                let _ = js_sys::Reflect::set(&object, &"bytes".into(), &buffer);
                let transfer = js_sys::Array::new();
                transfer.push(&buffer);
                let _ = out.post_message_with_transfer(&object, &transfer);
            } else if let Ok(text) = serde_json::to_string(&done) {
                let _ = out.post_message(&text.into());
            }
        }
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    scope.set_onmessage(Some(callback.as_ref().unchecked_ref()));
    callback.forget();
    let _ = scope.post_message(&"ready".into());
}
