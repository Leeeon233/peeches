# coding=utf-8
# Copyright 2018 The HuggingFace Inc. team.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
# https://github.com/huggingface/candle/blob/main/candle-examples/examples/marian-mt/convert_slow_tokenizer.py
"""
Utilities to convert slow tokenizers in their fast tokenizers counterparts.

All the conversions are grouped here to gather SentencePiece dependencies outside of the fast tokenizers files and
allow to make our dependency on SentencePiece optional.
"""

import warnings
from typing import Dict, List, Tuple

from packaging import version
from pathlib import Path
from tokenizers import AddedToken, Regex, Tokenizer, decoders, normalizers, pre_tokenizers, processors
from tokenizers.models import BPE, Unigram, WordPiece

from transformers.utils import is_protobuf_available, requires_backends
from transformers.utils.import_utils import PROTOBUF_IMPORT_ERROR


def import_protobuf(error_message=""):
    if is_protobuf_available():
        import google.protobuf

        if version.parse(google.protobuf.__version__) < version.parse("4.0.0"):
            from transformers.utils import sentencepiece_model_pb2
        else:
            from transformers.utils import sentencepiece_model_pb2_new as sentencepiece_model_pb2
        return sentencepiece_model_pb2
    else:
        raise ImportError(PROTOBUF_IMPORT_ERROR.format(error_message))

def _get_prepend_scheme(add_prefix_space: bool, original_tokenizer) -> str:
    if add_prefix_space:
        prepend_scheme = "always"
        if hasattr(original_tokenizer, "legacy") and not original_tokenizer.legacy:
            prepend_scheme = "first"
    else:
        prepend_scheme = "never"
    return prepend_scheme

class SentencePieceExtractor:
    """
    Extractor implementation for SentencePiece trained models. https://github.com/google/sentencepiece
    """

    def __init__(self, model: str):
        requires_backends(self, "sentencepiece")
        from sentencepiece import SentencePieceProcessor

        self.sp = SentencePieceProcessor()
        self.sp.Load(model)

    def extract(self, vocab_scores=None) -> Tuple[Dict[str, int], List[Tuple]]:
        """
        By default will return vocab and merges with respect to their order, by sending `vocab_scores` we're going to
        order the merges with respect to the piece scores instead.
        """
        sp = self.sp
        vocab = {sp.id_to_piece(index): index for index in range(sp.GetPieceSize())}
        if vocab_scores is not None:
            vocab_scores, reverse = dict(vocab_scores), True
        else:
            vocab_scores, reverse = vocab, False

        # Merges
        merges = []
        for merge, piece_score in vocab_scores.items():
            local = []
            for index in range(1, len(merge)):
                piece_l, piece_r = merge[:index], merge[index:]
                if piece_l in vocab and piece_r in vocab:
                    local.append((piece_l, piece_r, piece_score))
            local = sorted(local, key=lambda x: (vocab[x[0]], vocab[x[1]]))
            merges.extend(local)

        merges = sorted(merges, key=lambda val: val[2], reverse=reverse)
        merges = [(val[0], val[1]) for val in merges]
        return vocab, merges


def check_number_comma(piece: str) -> bool:
    return len(piece) < 2 or piece[-1] != "," or not piece[-2].isdigit()


class Converter:
    def __init__(self, original_tokenizer):
        self.original_tokenizer = original_tokenizer

    def converted(self) -> Tokenizer:
        raise NotImplementedError()


class SpmConverter(Converter):
    def __init__(self, *args):
        requires_backends(self, "protobuf")

        super().__init__(*args)

        # from .utils import sentencepiece_model_pb2 as model_pb2
        model_pb2 = import_protobuf()

        m = model_pb2.ModelProto()
        with open(self.original_tokenizer.vocab_file, "rb") as f:
            m.ParseFromString(f.read())
        self.proto = m

        if self.proto.trainer_spec.byte_fallback:
            if not getattr(self, "handle_byte_fallback", None):
                warnings.warn(
                    "The sentencepiece tokenizer that you are converting to a fast tokenizer uses the byte fallback option"
                    " which is not implemented in the fast tokenizers. In practice this means that the fast version of the"
                    " tokenizer can produce unknown tokens whereas the sentencepiece version would have converted these "
                    "unknown tokens into a sequence of byte tokens matching the original piece of text."
                )

    def vocab(self, proto):
        return [(piece.piece, piece.score) for piece in proto.pieces]

    def unk_id(self, proto):
        return proto.trainer_spec.unk_id

    def tokenizer(self, proto):
        model_type = proto.trainer_spec.model_type
        vocab_scores = self.vocab(proto)
        unk_id = self.unk_id(proto)

        if model_type == 1:
            tokenizer = Tokenizer(Unigram(vocab_scores, unk_id))
        elif model_type == 2:
            _, merges = SentencePieceExtractor(self.original_tokenizer.vocab_file).extract()
            bpe_vocab = {word: i for i, (word, score) in enumerate(vocab_scores)}
            tokenizer = Tokenizer(
                BPE(
                    bpe_vocab,
                    merges,
                    unk_token=proto.trainer_spec.unk_piece,
                    fuse_unk=True,
                )
            )
        else:
            raise Exception(
                "You're trying to run a `Unigram` model but you're file was trained with a different algorithm"
            )

        return tokenizer

    def normalizer(self, proto):
        precompiled_charsmap = proto.normalizer_spec.precompiled_charsmap
        if not precompiled_charsmap:
            return normalizers.Sequence([normalizers.Replace(Regex(" {2,}"), " ")])
        else:
            return normalizers.Sequence(
                [normalizers.Precompiled(precompiled_charsmap), normalizers.Replace(Regex(" {2,}"), " ")]
            )

    def pre_tokenizer(self, replacement, add_prefix_space):
        prepend_scheme = _get_prepend_scheme(add_prefix_space, self.original_tokenizer)
        return pre_tokenizers.Metaspace(replacement=replacement, prepend_scheme=prepend_scheme)

    def post_processor(self):
        return None

    def decoder(self, replacement, add_prefix_space):
        prepend_scheme = _get_prepend_scheme(add_prefix_space, self.original_tokenizer)
        return decoders.Metaspace(replacement=replacement, prepend_scheme=prepend_scheme)

    def converted(self) -> Tokenizer:
        tokenizer = self.tokenizer(self.proto)

        # Tokenizer assemble
        normalizer = self.normalizer(self.proto)
        if normalizer is not None:
            tokenizer.normalizer = normalizer

        replacement = "▁"
        add_prefix_space = True
        pre_tokenizer = self.pre_tokenizer(replacement, add_prefix_space)
        if pre_tokenizer is not None:
            tokenizer.pre_tokenizer = pre_tokenizer

        tokenizer.decoder = self.decoder(replacement, add_prefix_space)
        post_processor = self.post_processor()
        if post_processor:
            tokenizer.post_processor = post_processor

        return tokenizer

class MarianConverter(SpmConverter):
    def __init__(self, *args, index: int = 0):
        requires_backends(self, "protobuf")

        super(SpmConverter, self).__init__(*args)

        # from .utils import sentencepiece_model_pb2 as model_pb2
        model_pb2 = import_protobuf()

        m = model_pb2.ModelProto()
        print(self.original_tokenizer.spm_files)
        with open(self.original_tokenizer.spm_files[index], "rb") as f:
            m.ParseFromString(f.read())
        self.proto = m
        print(self.original_tokenizer)
        #with open(self.original_tokenizer.vocab_path, "r") as f:
        dir_path = Path(self.original_tokenizer.spm_files[0]).parents[0]
        with open(dir_path / "vocab.json", "r") as f:
            import json
            self._vocab = json.load(f)

        if self.proto.trainer_spec.byte_fallback:
            if not getattr(self, "handle_byte_fallback", None):
                warnings.warn(
                    "The sentencepiece tokenizer that you are converting to a fast tokenizer uses the byte fallback option"
                    " which is not implemented in the fast tokenizers. In practice this means that the fast version of the"
                    " tokenizer can produce unknown tokens whereas the sentencepiece version would have converted these "
                    "unknown tokens into a sequence of byte tokens matching the original piece of text."
                )

    def vocab(self, proto):
        vocab_size = max(self._vocab.values()) + 1
        vocab = [("<NIL>", -100) for _ in range(vocab_size)]
        for piece in proto.pieces:
            try:
                index = self._vocab[piece.piece]
            except Exception:
                print(f"Ignored missing piece {piece.piece}")
            vocab[index] = (piece.piece, piece.score)
        return vocab
