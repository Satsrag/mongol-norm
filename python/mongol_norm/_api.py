"""
Public API of ``mongol_norm``.

A thin wrapper over the Rust extension module :mod:`mongol_norm._native`
(the ``mongol-norm`` crate at the repository root) that preserves the pre-0.1
pure-Python API:
the same signatures, return shapes, exception types and messages. Argument
validation that produces Python-specific errors (``TypeError``, ``repr()``
formatting) happens here; shaping and encoding happen in Rust.
"""
import sys
from collections.abc import Sequence

from . import _native

__all__ = ["MongolianShaper", "NormalizationFallbackError", "main"]

MVS_CP = 0x180E
NIRUGU_CP = 0x180A
ZWJ_CP = 0x200D
# shape_detailed() reports structural tokens with these aliases (letters use
# their locale alias, or "" when the letter has none).
_STRUCTURAL_ALIASES = {MVS_CP: "mvs", NIRUGU_CP: "nirugu", ZWJ_CP: "zwj"}
_POSITIONED_UNIT_POSITIONS = frozenset({
    "isol", "init", "medi", "fina", "control",
})


class NormalizationFallbackError(ValueError):
    """Raised when strict normalization cannot encode a written-unit shape."""

    def __init__(self, text, written_units):
        self.text = text
        self.written_units = tuple(written_units)
        super().__init__(
            "normalization fallback: no canonical encoding for written units "
            + "+".join(self.written_units)
        )


def _call(fn, *args):
    """Call into the native module, translating its fallback signal."""
    try:
        return fn(*args)
    except _native.FallbackError as exc:
        text, written_units = exc.args
        raise NormalizationFallbackError(text, written_units) from None


class MongolianShaper:
    """
    Full UTN57 Hudum shaping engine.
    完整的 UTN57 回鹘式蒙古文字形引擎。

    The engine (the Rust crate ``mongol-norm``) carries the glyph variant data
    derived from the mongfontbuilder package, and provides three core operations:
    引擎（Rust crate ``mongol-norm``）内置源自 mongfontbuilder 的字形变体数据，
    提供三个核心操作：

      shape(text)      — Forward shaping: text → visual glyph sequence
                         正向字形处理：文本 → 视觉字形序列
      same_shape(a, b) — Visual identity comparison (do a and b look the same?)
                         视觉一致性比较（a 和 b 看起来一样吗？）
      normalize(text)  — Canonical encoding: many-to-one mapping via shape+harmony
                         规范化编码：通过字形+元音和谐实现多对一映射

    Usage / 用法:
        shaper = MongolianShaper(locale="MNG")
        shape = shaper.shape("ᠰᠠᠢᠨ")  # → ['S', 'A', 'I', 'I', 'A']
    """

    def __init__(self, locale="MNG"):
        self._native = _native.Shaper(locale)
        self.locale = locale

    @classmethod
    def _wrap(cls, native):
        """Wrap a ready ``_native.Shaper`` (test hooks)."""
        self = cls.__new__(cls)
        self._native = native
        self.locale = native.locale
        return self

    # ── Shaping ─────────────────────────────────────────────────

    def shape(self, text):
        """
        Shape *text* into its written-unit sequence.
        将 *text* 处理为书写单元序列。

        Raises ValueError on characters outside the Mongolian word alphabet
        (letters, FVS, MVS, NNBSP, Nirugu, ZWJ); use :meth:`normalize_text`
        for mixed-script input.
        """
        return self._native.shape(text)

    def shape_str(self, text):
        """``"+".join(shape(text))``."""
        return self._native.shape_str(text)

    def same_shape(self, text1, text2):
        """Do *text1* and *text2* render identically?"""
        return self._native.same_shape(text1, text2)

    def shape_detailed(self, text):
        """Return detailed shaping breakdown per token. Raises on non-Mongolian input."""
        details = []
        for cp, alias, position, fvs, condition, written in (
                self._native.shape_detailed(text)):
            if alias is None:
                alias = _STRUCTURAL_ALIASES.get(cp, "")
            details.append({
                "cp": f"U+{cp:04X}",
                "alias": alias,
                "position": position,
                "fvs": f"+FVS{fvs}" if fvs else "",
                "condition": condition or "",
                "written": written,
            })
        return details

    def trace(self, text):
        """
        Trace the rule pipeline over *text* (the phase-trace golden format).

        Returns ``{"positions", "transitions", "final_conditions",
        "written_by_token", "shape"}``; each transition is
        ``{"rule": name, "changes": [{"token", "before", "after"}, ...]}`` and
        lists only rules that changed at least one condition.
        """
        positions, transitions, final_conditions, written_by_token, shape = (
            self._native.trace(text))
        return {
            "positions": positions,
            "transitions": [
                {
                    "rule": rule,
                    "changes": [
                        {"token": token, "before": before, "after": after}
                        for token, before, after in changes
                    ],
                }
                for rule, changes in transitions
            ],
            "final_conditions": final_conditions,
            "written_by_token": written_by_token,
            "shape": shape,
        }

    def rule_names(self):
        """Names of the shaping rules for this locale, in pipeline order."""
        return self._native.rule_names()

    # ── Normalization ───────────────────────────────────────────

    @property
    def canonical_version(self):
        """Version of the canonical Unicode selection policy for this locale."""
        return self._native.canonical_version()

    def normalize(self, text, strict=True):
        """
        Canonical encoding of a single Mongolian word.
        单个蒙古文词的规范化编码。

        With ``strict=True`` (default) a shape the normalize table cannot
        encode raises :class:`NormalizationFallbackError`; with
        ``strict=False`` the input is returned unchanged instead. Raises
        ValueError on non-Mongolian characters (see :meth:`normalize_text`).
        """
        return _call(self._native.normalize, text, strict)

    def normalize_text(self, text, strict=True):
        """
        Normalize every Mongolian word run inside free-form *text*, leaving
        other characters untouched.
        对自由文本中的每个蒙古文词段做规范化，其余字符原样保留。
        """
        return _call(self._native.normalize_text, text, strict)

    def normalize_written_units(self, written_units):
        """
        Encode an ordered written-unit sequence as canonical MNG Unicode.
        将有序书写单元序列编码为 canonical MNG Unicode。

        ``written_units`` may be the direct output of :meth:`shape`. Structural
        written-unit names are PascalCase: ``Mvs``, ``Nirugu``, and ``Zwj``. Letter
        positions are inferred from sequence order and structural controls; this
        API does not accept explicit position records or infer/insert controls.
        ZWJ is emitted only when ``Zwj`` is present in the request. An
        empty sequence returns an empty string.
        ``written_units`` 可直接使用 :meth:`shape` 的输出。外部调用方也可将结构
        token 写作 PascalCase：``Mvs``、``Nirugu``、``Zwj``。字母位置由序列顺序
        与结构 control 推导；本 API 不接受显式位置 record，也不推断或插入
        control。只有请求含 ``Zwj`` 时才输出
        ZWJ；空序列返回空字符串。

        The result is accepted only when it reshapes to the exact requested
        sequence. An unknown/malformed unit or an unencodable sequence raises
        instead of guessing or returning a partial result.
        仅当输出重新 shape 后与请求序列完全一致时才接受。未知/非法 unit 或无法
        编码的序列会抛出异常，不猜测，也不返回部分结果。

        Raises:
            TypeError: ``written_units`` is not an ordered string sequence.
            ValueError: a unit is unknown or the sequence cannot be encoded
                with an exact shape round trip.
        """
        if (isinstance(written_units, (str, bytes))
                or not isinstance(written_units, Sequence)):
            raise TypeError("written_units must be an ordered sequence of strings")
        target = []
        for index, unit in enumerate(written_units):
            if not isinstance(unit, str):
                raise TypeError(f"written_units[{index}] must be a string")
            target.append(unit)
        if not target:
            return ""
        known_units = self._known_units()
        for index, unit in enumerate(target):
            if unit not in known_units:
                raise ValueError(f"written_units[{index}] is unknown: {unit!r}")
        return self._native.normalize_written_units(target)

    def normalize_positioned_written_units(self, positioned_units):
        """
        Encode explicit-position records as canonical MNG Unicode.
        将显式位置 record 编码为 canonical MNG Unicode。

        Each item must be exactly a built-in
        ``{"unit": str, "position": str}`` dict. Letter positions are the
        authoritative HUD written-unit positions ``isol``/``init``/``medi``/
        ``fina``. Encoding delegates to :meth:`normalize_written_units`: a
        complete multi-record chain runs from ``init`` to ``fina``; an incomplete
        edge gets an implicit ZWJ. A single ``init`` record is normally encoded
        without ZWJ; the sole ``O:init`` exception receives a trailing ZWJ. Single
        ``medi`` and ``fina`` records receive the joining context their positions
        need. ``Mvs`` and ``Nirugu`` require ``control``; explicit ``Zwj`` input is
        rejected.

        每项必须严格为内建 ``{"unit": str, "position": str}`` dict。字母position
        是权威HUD written-unit position：``isol``/``init``/``medi``/``fina``。
        编码直接交给 :meth:`normalize_written_units`：完整复合链从``init``开始、
        到``fina``结束；边界不完整时自动补ZWJ。单个``init``通常不补ZWJ；唯一
        特例``O:init``在末尾补ZWJ。单个``medi``和``fina``补足其位置所需的连接
        上下文。``Mvs``与``Nirugu``使用``control``；显式``Zwj``输入被拒绝。

        This word-level API accepts at most 1024 records.

        Raises:
            TypeError: the outer value, a record, or a field has the wrong type.
            ValueError: record keys, unit, position, contextual position, or exact
                canonical encoding is invalid.
        """
        if (isinstance(positioned_units, (str, bytes))
                or not isinstance(positioned_units, Sequence)):
            raise TypeError(
                "positioned_units must be an ordered sequence of records"
            )
        if len(positioned_units) > 1024:
            raise ValueError("positioned_units accepts at most 1024 records")
        records = []
        for index, record in enumerate(positioned_units):
            if type(record) is not dict:
                raise TypeError(f"positioned_units[{index}] must be a record")
            if set(record) != {"unit", "position"}:
                raise ValueError(
                    f"positioned_units[{index}] must contain exactly "
                    "'unit' and 'position'"
                )
            unit = record["unit"]
            position = record["position"]
            if not isinstance(unit, str):
                raise TypeError(
                    f"positioned_units[{index}].unit must be a string"
                )
            if not isinstance(position, str):
                raise TypeError(
                    f"positioned_units[{index}].position must be a string"
                )
            if position not in _POSITIONED_UNIT_POSITIONS:
                raise ValueError(
                    f"positioned_units[{index}] has unknown position "
                    f"{position!r}"
                )
            records.append((unit, position))
        if any(unit == "Zwj" for unit, _position in records):
            raise ValueError("unsupported positioned control 'Zwj'")
        if not records:
            return ""
        positioned_units_inventory = self._positioned_units()
        for index, (unit, position) in enumerate(records):
            if unit in ("Mvs", "Nirugu"):
                if position != "control":
                    raise ValueError(
                        f"positioned_units[{index}] control {unit!r} "
                        "requires position 'control'"
                    )
                continue
            if (unit, position) not in positioned_units_inventory:
                raise ValueError(
                    "unsupported positioned written unit "
                    f"{unit + ':' + position!r}"
                )
        return self._native.normalize_positioned_written_units(records)

    def parse_written_units(self, text):
        """
        Parse the CLI spelling of a written-unit sequence (``A+B`` or the
        compact ``AB`` form) into unit names; raises ValueError when it is
        malformed or names a unit the normalize table does not know.
        """
        return self._native.parse_written_units(text)

    # ── Inventories (cached; RuntimeError for locales without a table) ──

    def _known_units(self):
        try:
            return self._known_units_cache
        except AttributeError:
            self._known_units_cache = frozenset(self._native.known_written_units())
            return self._known_units_cache

    def _positioned_units(self):
        try:
            return self._positioned_units_cache
        except AttributeError:
            self._positioned_units_cache = frozenset(
                self._native.positioned_written_units())
            return self._positioned_units_cache


def main():
    """Console-script entry point: the ``mongol-norm`` CLI (see ``--help``)."""
    sys.stdout.flush()
    sys.stderr.flush()
    sys.exit(_native.cli_main(sys.argv[1:]))
