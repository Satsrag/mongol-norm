// Static reference pages: translation never changes glyphs, code points, or source excerpts.
(() => {
  const glossary = [
    [["ZWNJ", "Zwnj"], "U+200C, zero width non-joiner. Breaks the joining chain. The proposed pass tests one removal at a time.", "U+200C，零宽非连接符，断开连接链。拟实现的流程每次模拟删除一个。"],
    [["ZWJ", "Zwj"], "U+200D, zero width joiner. Provides a connection. ZWJ drop decides whether it is redundant.", "U+200D，零宽连接符，提供连接。由 ZWJ drop 判断是否冗余。"],
    [["Nirugu"], "U+180A, Mongolian nirugu. Provides a visible joining stroke; preserve its count and order.", "U+180A，蒙古文连接笔画。参与连接，保留数量与顺序。"],
    [["MVS", "Mvs"], "U+180E, Mongolian vowel separator. Preserves a segment boundary.", "U+180E，蒙古文元音分隔符，保留分段边界。"],
    [["letter", "letters", "字母单元", "字母"], "A written unit other than Mvs, Nirugu, Zwj, or Zwnj.", "除 Mvs、Nirugu、Zwj、Zwnj 之外的 written unit。"],
    [["boundary", "boundaries", "边界"], "Start or end of the sequence, or an Mvs or Zwnj unit.", "序列开头／结尾，或 Mvs、Zwnj。"],
    [["outer", "far side", "外侧"], "The side away from the control being tested: left neighbor of the left letter, right neighbor of the right letter.", "远离待删控制符的一侧：左字母看左邻，右字母看右邻；只看直接相邻单元。"],
    [["joiner", "joiners", "连接符"], "Here, ZWJ or Nirugu. Both affect joining positions; Nirugu also has a visible stroke.", "这里指 ZWJ 或 Nirugu，都参与连接位置计算；Nirugu 还保留可见笔画。"],
    [["connected", "connection", "仍有连接", "有连接"], "The immediate neighbor is a letter, ZWJ, or Nirugu.", "直接相邻的是字母、ZWJ 或 Nirugu。"],
    [["position", "positions", "位置"], "isol/init/medi/fina are computed from the current chain. ZWJ and Nirugu connect; MVS and ZWNJ break the chain.", "按当前单元链计算 isol/init/medi/fina；ZWJ、Nirugu 提供连接，MVS、ZWNJ 断链。"],
    [["isol"], "Isolated form.", "独立形。"],
    [["init"], "Initial form.", "开头形。"],
    [["medi"], "Medial form.", "中间形。"],
    [["fina"], "Final form.", "结尾形。"],
    [["INITIAL_BEFORE_LETTER"], "medi ↔ init equivalence. For ZWJ removal at a segment start, the following letter remains connected on its far side.", "medi ↔ init 等价表。删除片段开头的 ZWJ 时，后方字母外侧仍有连接。"],
    [["INITIAL_ALONE"], "fina ↔ isol equivalence. For ZWJ removal at a segment start, the following letter has a boundary on its far side.", "fina ↔ isol 等价表。删除片段开头的 ZWJ 时，后方字母外侧是边界。"],
    [["FINAL_AFTER_LETTER"], "medi ↔ fina equivalence. For ZWJ removal at a segment end, the preceding letter remains connected on its far side.", "medi ↔ fina 等价表。删除片段结尾的 ZWJ 时，前方字母外侧仍有连接。"],
    [["FINAL_ALONE"], "init ↔ isol equivalence. For ZWJ removal at a segment end, the preceding letter has a boundary on its far side.", "init ↔ isol 等价表。删除片段结尾的 ZWJ 时，前方字母外侧是边界。"],
    [["INIT_TO_MEDI"], "init ↔ medi equivalence. For ZWNJ removal, the right letter has a connection on its far side and changes from init to medi.", "init ↔ medi 等价表。删除 ZWNJ 时，右字母外侧有连接，位置由 init 变为 medi。"],
    [["FINA_TO_MEDI"], "fina ↔ medi equivalence. For ZWNJ removal, the left letter has a connection on its far side and changes from fina to medi.", "fina ↔ medi 等价表。删除 ZWNJ 时，左字母外侧有连接，位置由 fina 变为 medi。"],
    [["ISOL_TO_FINA"], "isol ↔ fina equivalence. For ZWNJ removal, the right letter has a boundary on its far side and changes from isol to fina.", "isol ↔ fina 等价表。删除 ZWNJ 时，右字母外侧是边界，位置由 isol 变为 fina。"],
    [["ISOL_TO_INIT"], "isol ↔ init equivalence. For ZWNJ removal, the left letter has a boundary on its far side and changes from isol to init.", "isol ↔ init 等价表。删除 ZWNJ 时，左字母外侧是边界，位置由 isol 变为 init。"],
    [["bowed written unit", "bowed written units", "bowed unit", "bowed units", "弓形单元"], "One of the 7 Hudum bowed consonant units: B, P, F, G, Gx, K, K2.", "7 个胡都木弓形辅音单元之一：B、P、F、G、Gx、K、K2。"],
  ];
  const terms = new Map(glossary.flatMap(([names, en, zh]) => names.map(name => [name.toLowerCase(), { en, zh }])));
  const escape = value => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp([...terms.keys()].sort((a, b) => b.length - a.length).map(term => /^[\x00-\x7f]+$/.test(term) ? `(?<![a-z0-9_])${escape(term)}(?![a-z0-9_])` : escape(term)).join("|"), "gi");
  const translated = [...document.querySelectorAll("i18n-text")];
  const attributes = [...document.querySelectorAll("[data-en-aria-label], [data-en-title]")];
  const buttons = [...document.querySelectorAll("[data-language]")];
  function annotate(language) {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
      acceptNode: node => node.parentElement.closest("script, style, pre, svg, button, .term, .mongol, .brand, .meta, .side-note") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT,
    });
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const node of nodes) {
      const matches = [...node.data.matchAll(pattern)];
      if (!matches.length) continue;
      const fragment = document.createDocumentFragment();
      let offset = 0;
      for (const match of matches) {
        fragment.append(node.data.slice(offset, match.index));
        const term = document.createElement("span");
        term.className = "term";
        term.tabIndex = 0;
        term.dataset.term = match[0].toLowerCase();
        term.dataset.tooltip = terms.get(term.dataset.term)[language];
        term.textContent = match[0];
        fragment.append(term);
        offset = match.index + match[0].length;
      }
      fragment.append(node.data.slice(offset));
      node.replaceWith(fragment);
    }
    for (const term of document.querySelectorAll("[data-term]")) term.dataset.tooltip = terms.get(term.dataset.term)[language];
  }
  function setLanguage(language) {
    language = language === "zh" ? "zh" : "en";
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
    for (const element of translated) element.textContent = element.dataset[language];
    for (const element of attributes) {
      for (const name of ["aria-label", "title"]) {
        const value = element.getAttribute(`data-${language}-${name}`);
        if (value !== null) element.setAttribute(name, value);
      }
    }
    for (const button of buttons) button.setAttribute("aria-pressed", String(button.dataset.language === language));
    // Close any tooltip whose term has just been translated.
    dispatchEvent(new Event("resize"));
    if (document.getElementById("term-tooltip")) annotate(language);
    try { localStorage.setItem("mongol-norm-rules-language", language); } catch {}
  }
  for (const button of buttons) button.addEventListener("click", () => setLanguage(button.dataset.language));
  let preference = "en";
  try { preference = localStorage.getItem("mongol-norm-rules-language") || "en"; } catch {}
  setLanguage(new URLSearchParams(location.search).get("lang") || preference);
})();
