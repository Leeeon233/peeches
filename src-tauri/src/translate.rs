use anyhow::Error as E;
use candle_core::{DType, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::marian::{self, MTModel};
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

#[derive(Clone)]
pub struct Translator {
    inner: Arc<Mutex<TranslatorInner>>,
}

impl Translator {
    pub fn new(model: &str, en_token: &str, zh_token: &str) -> anyhow::Result<Self> {
        let inner = TranslatorInner::new(model, en_token, zh_token)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub fn translate(&self, text: &str) -> anyhow::Result<String> {
        let mut inner = self.inner.lock().unwrap();
        inner.translate(text)
    }
}
// struct TranslatorInner {}
// impl TranslatorInner {
//     fn new(model: &str, en_token: &str, zh_token: &str) -> anyhow::Result<Self> {
//         Ok(Self {})
//     }

//     fn translate(&self, text: &str) -> anyhow::Result<String> {
//         Ok(String::new())
//     }
// }

struct TranslatorInner {
    model: MTModel,
    config: marian::Config,
    tokenizer: Tokenizer,
    tokenizer_dec: Tokenizer,
    device: candle_core::Device,
}

impl TranslatorInner {
    fn new(model: &str, en_token: &str, zh_token: &str) -> anyhow::Result<Self> {
        let tokenizer = Tokenizer::from_file(en_token).map_err(E::msg)?;
        let tokenizer_dec = Tokenizer::from_file(zh_token).map_err(E::msg)?;
        // let tokenizer_dec = TokenOutputStream::new(tokenizer_dec);
        let device = if cfg!(target_os="macos"){
            candle_core::Device::new_metal(0)?
        } else{
            candle_core::Device::new_cuda(0)?
        };
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&model], DType::F32, &device)? };
        // https://huggingface.co/Helsinki-NLP/opus-mt-en-zh/blob/main/config.json
        let config = marian::Config {
            vocab_size: 65001,
            decoder_vocab_size: Some(65001),
            max_position_embeddings: 512,
            encoder_layers: 6,
            encoder_ffn_dim: 2048,
            encoder_attention_heads: 8,
            decoder_layers: 6,
            decoder_ffn_dim: 2048,
            decoder_attention_heads: 8,
            use_cache: true,
            is_encoder_decoder: true,
            activation_function: candle_nn::Activation::Swish,
            d_model: 512,
            decoder_start_token_id: 65000,
            scale_embedding: true,
            pad_token_id: 65000,
            eos_token_id: 0,
            forced_eos_token_id: 0,
            share_encoder_decoder_embeddings: true,
        };
        let model = marian::MTModel::new(&config, vb)?;
        Ok(Self {
            model,
            config,
            tokenizer,
            tokenizer_dec,
            device,
        })
    }

    fn translate(&mut self, text: &str) -> anyhow::Result<String> {
        let mut logits_processor =
            candle_transformers::generation::LogitsProcessor::new(1337, None, None);
        let encoder_xs = {
            let mut tokens = self
                .tokenizer
                .encode(text, true)
                .map_err(E::msg)?
                .get_ids()
                .to_vec();
            tokens.push(self.config.eos_token_id);
            let tokens = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;
            self.model.encoder().forward(&tokens, 0)?
        };
        let mut token_ids = vec![self.config.decoder_start_token_id];
        for index in 0..1000 {
            let context_size = if index >= 1 { 1 } else { token_ids.len() };
            let start_pos = token_ids.len().saturating_sub(context_size);
            let input_ids = Tensor::new(&token_ids[start_pos..], &self.device)?.unsqueeze(0)?;
            let logits = self.model.decode(&input_ids, &encoder_xs, start_pos)?;
            let logits = logits.squeeze(0)?;
            let logits = logits.get(logits.dim(0)? - 1)?;
            let token = logits_processor.sample(&logits)?;
            if token == self.config.eos_token_id || token == self.config.forced_eos_token_id {
                break;
            }
            token_ids.push(token);
            // if let Some(t) = self.tokenizer_dec.next_token(token)? {
            //     ans.push_str(&t);
            // }
        }
        // if let Some(rest) = self.tokenizer_dec.decode_rest().map_err(E::msg)? {
        //     ans.push_str(&rest);
        // }
        let ans = self
            .tokenizer_dec
            .decode(&token_ids[1..], true)
            .map_err(E::msg)?;
        self.model.reset_kv_cache();
        Ok(ans)
    }
}
