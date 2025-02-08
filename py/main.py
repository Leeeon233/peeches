from convert import MarianConverter
from transformers import AutoTokenizer


tokenizer = AutoTokenizer.from_pretrained("Helsinki-NLP/opus-mt-en-zh", use_fast=False)
fast_tokenizer = MarianConverter(tokenizer, index=0).converted()
fast_tokenizer.save(f"tokenizer-marian-base-en.json")
fast_tokenizer = MarianConverter(tokenizer, index=1).converted()
fast_tokenizer.save(f"tokenizer-marian-base-zh.json")