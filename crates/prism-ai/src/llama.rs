//! The engine: llama.cpp (MIT) through the llama-cpp-2 bindings, built from source with the app. It reads the GGUF
//! model file (and its tokenizer, which is inside it), and decoding is constrained by [`crate::model`]'s filter so only
//! prose and real `{slot}` names can be written. llama.cpp processes the prompt in batches; the candle engine used in
//! the first spike took about eight times as long for the same paragraph. Building needs CMake and libclang (README).

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::token::LlamaToken;

use crate::model::{allowed, installed, MODEL};
use crate::slots::Input;

struct Loaded {
    dir: std::path::PathBuf,
    model: LlamaModel,
    texts: Vec<String>,
    end: LlamaToken,
}

fn backend() -> Result<&'static LlamaBackend, String> {
    static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();
    BACKEND
        .get_or_init(|| {
            let mut b = LlamaBackend::init().map_err(|e| format!("The AI engine could not start: {e}"))?;
            b.void_logs();
            Ok(b)
        })
        .as_ref()
        .map_err(Clone::clone)
}

static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

fn threads() -> i32 {
    // Physical cores do the work; hyperthreads add little to matrix kernels.
    std::thread::available_parallelism().map_or(4, |n| (n.get() / 2).max(1)) as i32
}

pub fn generate(dir: &Path, input: &Input) -> Result<(String, f64), String> {
    if !installed(dir) {
        return Err("The AI model isn't installed. Turn on AI writing and download the model first.".into());
    }
    let backend = backend()?;
    let mut guard = LOADED.lock().map_err(|_| "The AI writer stopped after an earlier failure. Restart PrismStatistics.".to_string())?;
    if guard.as_ref().is_none_or(|l| l.dir != dir) {
        let model = LlamaModel::load_from_file(backend, dir.join(MODEL.files[0].name), &LlamaModelParams::default()).map_err(|e| format!("The AI model could not be loaded: {e}"))?;
        let vocab = model.vocab();
        let texts = (0..vocab.n_tokens()).map(|i| String::from_utf8_lossy(&vocab.token_to_piece(LlamaToken(i), false, None)).into_owned()).collect();
        let end = vocab.tokenize(b"<|im_end|>", false, true).first().copied().ok_or("The tokenizer has no end token.")?;
        *guard = Some(Loaded { dir: dir.to_path_buf(), model, texts, end });
    }
    let l = guard.as_ref().expect("loaded above");
    let start = Instant::now();
    let fail = |e: &dyn std::fmt::Display| format!("The AI model stopped while writing: {e}");
    let params = LlamaContextParams::default().with_n_ctx(NonZeroU32::new(4096)).with_n_batch(1024).with_n_threads(threads()).with_n_threads_batch(threads());
    let mut ctx = l.model.new_context(backend, params).map_err(|e| fail(&e))?;
    let prompt = format!("<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n", input.system, input.user);
    let ids = l.model.vocab().tokenize(prompt.as_bytes(), false, true);
    let mut batch = LlamaBatch::new(ids.len().max(1), 1);
    for (i, t) in ids.iter().enumerate() {
        batch.add(*t, i as i32, &[0], i + 1 == ids.len()).map_err(|e| fail(&e))?;
    }
    ctx.decode(&mut batch).map_err(|e| fail(&e))?;
    let names: Vec<&str> = input.slots.iter().map(|s| s.name.as_str()).collect();
    let mut out = String::new();
    let mut last = batch.n_tokens() - 1;
    for pos in ids.len()..ids.len() + 900 {
        let scores = ctx.get_logits_ith(last);
        let mut order: Vec<usize> = (0..scores.len().min(l.texts.len())).collect();
        order.sort_unstable_by(|a, b| scores[*b].total_cmp(&scores[*a]));
        let open_slot = out.rfind('{').is_some_and(|i| !out[i..].contains('}'));
        let next = order.into_iter().find(|&i| (LlamaToken(i as i32) == l.end && !open_slot) || allowed(&out, &l.texts[i], &names)).ok_or("The AI model couldn't continue the paragraph.")?;
        if LlamaToken(next as i32) == l.end {
            break;
        }
        out.push_str(&l.texts[next]);
        batch.clear();
        batch.add(LlamaToken(next as i32), pos as i32, &[0], true).map_err(|e| fail(&e))?;
        ctx.decode(&mut batch).map_err(|e| fail(&e))?;
        last = 0;
    }
    Ok((out.trim().to_string(), start.elapsed().as_secs_f64()))
}
