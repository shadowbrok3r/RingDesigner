use crate::job::{Done, Job};

#[cfg(not(target_arch = "wasm32"))]
pub struct Worker {
    tx: std::sync::mpsc::Sender<Job>,
    rx: std::sync::mpsc::Receiver<Result<Done, String>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Worker {
    pub fn new(ctx: egui::Context) -> Result<Self, String> {
        let (tx, jobs) = std::sync::mpsc::channel::<Job>();
        let (out, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("workshop".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::job::process(job)
                    }))
                    .map_err(|_| {
                        "Geometry worker failed; change the source or reopen the project".into()
                    });
                    if out.send(result).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, rx })
    }
    pub fn send(&mut self, job: Job) -> Result<(), String> {
        self.tx.send(job).map_err(|e| e.to_string())
    }
    pub fn poll(&mut self) -> Option<Result<Done, String>> {
        self.rx.try_recv().ok()
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, closure::Closure};
    pub struct Worker {
        worker: web_sys::Worker,
        queue: Rc<RefCell<VecDeque<Result<Done, String>>>>,
        ready: Rc<Cell<bool>>,
        pending: Rc<RefCell<Option<Job>>>,
        _message: Closure<dyn FnMut(web_sys::MessageEvent)>,
        _error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    }
    impl Worker {
        pub fn new(ctx: egui::Context) -> Result<Self, String> {
            let worker = web_sys::Worker::new("workshop-worker_loader.js")
                .map_err(|e| format!("Cannot start workshop worker: {e:?}"))?;
            let queue = Rc::new(RefCell::new(VecDeque::new()));
            let ready = Rc::new(Cell::new(false));
            let pending = Rc::new(RefCell::new(None::<Job>));
            let (q, r, p, w, c) = (
                queue.clone(),
                ready.clone(),
                pending.clone(),
                worker.clone(),
                ctx.clone(),
            );
            let message = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
                if let Some(text) = event.data().as_string() {
                    if text == "ready" {
                        r.set(true);
                        if let Some(job) = p.borrow_mut().take() {
                            if let Err(e) = send(&w, &job) {
                                q.borrow_mut().push_back(Err(e));
                            }
                        }
                    } else {
                        q.borrow_mut()
                            .push_back(serde_json::from_str(&text).map_err(|e| e.to_string()));
                    }
                    c.request_repaint();
                } else {
                    // Large packages cross the boundary as transferred binary buffers.
                    // Parsing a JSON array of every STL/ZIP byte would block the UI.
                    let data = event.data();
                    let read = |name: &str| {
                        js_sys::Reflect::get(&data, &name.into()).map_err(|e| format!("{e:?}"))
                    };
                    let result = (|| -> Result<Done, String> {
                        let string = |name: &str| {
                            read(name)?
                                .as_string()
                                .ok_or_else(|| format!("Missing {name}"))
                        };
                        let id = string("id")?.parse::<u64>().map_err(|e| e.to_string())?;
                        let key = string("key")?.parse::<u64>().map_err(|e| e.to_string())?;
                        let artifact = crate::job::Artifact {
                            name: string("name")?,
                            mime: string("mime")?,
                            bytes: js_sys::Uint8Array::new(&read("bytes")?).to_vec(),
                        };
                        Ok(Done {
                            id,
                            key,
                            result: Ok(crate::job::Output::Artifact(artifact)),
                        })
                    })();
                    q.borrow_mut().push_back(result);
                    c.request_repaint();
                }
            }) as Box<dyn FnMut(web_sys::MessageEvent)>);
            worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
            let q = queue.clone();
            let error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
                q.borrow_mut()
                    .push_back(Err(format!("Workshop worker: {}", event.message())));
                ctx.request_repaint();
            }) as Box<dyn FnMut(web_sys::ErrorEvent)>);
            worker.set_onerror(Some(error.as_ref().unchecked_ref()));
            Ok(Self {
                worker,
                queue,
                ready,
                pending,
                _message: message,
                _error: error,
            })
        }
        pub fn send(&mut self, job: Job) -> Result<(), String> {
            if self.ready.get() {
                send(&self.worker, &job)
            } else {
                *self.pending.borrow_mut() = Some(job);
                Ok(())
            }
        }
        pub fn poll(&mut self) -> Option<Result<Done, String>> {
            self.queue.borrow_mut().pop_front()
        }
    }
    fn send(worker: &web_sys::Worker, job: &Job) -> Result<(), String> {
        worker
            .post_message(
                &serde_json::to_string(job)
                    .map_err(|e| e.to_string())?
                    .into(),
            )
            .map_err(|e| format!("{e:?}"))
    }
    impl Drop for Worker {
        fn drop(&mut self) {
            self.worker.terminate();
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use browser::Worker;
