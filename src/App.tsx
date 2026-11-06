import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  Clipboard,
  Code2,
  Copy,
  FileText,
  FolderSync,
  Image as ImageIcon,
  Keyboard,
  Link2,
  PauseCircle,
  Pin,
  PlayCircle,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Smartphone,
  Sparkles,
  Trash2,
  Wand2,
  Zap,
} from "lucide-react";
import {
  addManualClip,
  applyCleanToItem,
  clearUnpinnedClips,
  copyToOsClipboard,
  deleteClip,
  deleteSnippetCmd,
  expandAndCopySnippet,
  expandTextPreview,
  getAppSettings,
  getClipboardHistory,
  getSnippets,
  hideMainWindow,
  onClipboardUpdated,
  onGlobalShortcutTriggered,
  saveAppSettings,
  saveSnippetCmd,
  simulateRemoteDeviceSync,
  syncNowCmd,
  togglePinClip,
  transformTextCmd,
} from "./api";
import type {
  AppSettings,
  CleanTransformResult,
  ClipCategory,
  ClipboardItem,
  Snippet,
  SyncReport,
} from "./types";

type ActiveTab = "history" | "snippets" | "cleaner" | "sync";

export function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>("history");
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  // Clipboard History state
  const [clips, setClips] = useState<ClipboardItem[]>([]);
  const [categoryFilter, setCategoryFilter] = useState<string>("all");
  const [pinnedOnly, setPinnedOnly] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedClipId, setSelectedClipId] = useState<string | null>(null);
  const [newClipText, setNewClipText] = useState<string>("");
  const [showNewClipModal, setShowNewClipModal] = useState<boolean>(false);

  // Snippets state
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [editingSnippet, setEditingSnippet] = useState<{
    id?: string | null;
    trigger: string;
    title: string;
    body: string;
    category: string;
    enabled: boolean;
  }>({
    id: null,
    trigger: ";",
    title: "",
    body: "",
    category: "General",
    enabled: true,
  });
  const [snippetSandboxInput, setSnippetSandboxInput] = useState<string>(
    "Escribe aquí un atajo como ;firma o ;fecha para probar la expansión automática..."
  );
  const [snippetSandboxOutput, setSnippetSandboxOutput] = useState<string>("");
  const [snippetSandboxTriggers, setSnippetSandboxTriggers] = useState<
    string[]
  >([]);

  // Cleaner Lab state
  const [cleanerInput, setCleanerInput] = useState<string>(
    '{"proyecto":"UltiCP","url":"https://ejemplo.es/producto?id=99&utm_source=instagram&utm_medium=cpc&fbclid=IwAR1234","plataformas":["Windows","Android"]}'
  );
  const [cleanerResult, setCleanerResult] =
    useState<CleanTransformResult | null>(null);
  const [optCleanUrls, setOptCleanUrls] = useState<boolean>(true);
  const [optPrettifyJson, setOptPrettifyJson] = useState<boolean>(true);
  const [optMinifyJson, setOptMinifyJson] = useState<boolean>(false);
  const [optCleanMarkdown, setOptCleanMarkdown] = useState<boolean>(true);

  // Settings & Sync state
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [lastSyncReport, setLastSyncReport] = useState<SyncReport | null>(null);

  // Toast feedback
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => {
      setToast((prev) => (prev === msg ? null : prev));
    }, 2800);
  };

  const refreshHistory = async () => {
    try {
      const list = await getClipboardHistory(
        categoryFilter,
        searchQuery,
        pinnedOnly
      );
      setClips(list);
      if (list.length > 0 && !selectedClipId) {
        setSelectedClipId(list[0].id);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const refreshSnippets = async () => {
    try {
      const list = await getSnippets();
      setSnippets(list);
    } catch (err) {
      console.error(err);
    }
  };

  const refreshSettings = async () => {
    try {
      const s = await getAppSettings();
      setSettings(s);
    } catch (err) {
      console.error(err);
    }
  };

  useEffect(() => {
    refreshHistory();
  }, [categoryFilter, searchQuery, pinnedOnly]);

  useEffect(() => {
    refreshSnippets();
    refreshSettings();

    let cleanupClip: (() => void) | undefined;
    let cleanupShortcut: (() => void) | undefined;

    onClipboardUpdated((item) => {
      refreshHistory();
      setSelectedClipId(item.id);
      showToast(`Capturado en portapapeles (${item.category})`);
    }).then((unlisten) => {
      cleanupClip = unlisten;
    });

    onGlobalShortcutTriggered((action) => {
      if (action === "focus-search") {
        setActiveTab("history");
        refreshHistory();
        setTimeout(() => {
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
        }, 40);
      } else if (action === "open-snippets") {
        setActiveTab("snippets");
        refreshSnippets();
        showToast("Snippets abiertos vía atajo global");
      } else if (action === "clipboard-cleaned") {
        refreshHistory();
        showToast("Portapapeles limpiado y expandido al vuelo");
      } else if (action === "synced") {
        refreshHistory();
        refreshSnippets();
        showToast("Sincronizado con carpeta Syncthing");
      }
    }).then((unlisten) => {
      cleanupShortcut = unlisten;
    });

    return () => {
      if (cleanupClip) cleanupClip();
      if (cleanupShortcut) cleanupShortcut();
    };
  }, []);

  const selectedClip = useMemo(
    () => clips.find((c) => c.id === selectedClipId) ?? clips[0] ?? null,
    [clips, selectedClipId]
  );

  // In-window keyboard shortcuts (↑/↓ navigation, Enter copy, Shift+Enter clean copy, Ctrl+K search, Esc hide)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+K: Focus search box from anywhere
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setActiveTab("history");
        setTimeout(() => {
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
        }, 20);
        return;
      }

      // Escape: Clear search or hide window to tray
      if (e.key === "Escape") {
        if (showNewClipModal) {
          setShowNewClipModal(false);
          return;
        }
        if (searchQuery.trim() !== "") {
          setSearchQuery("");
          return;
        }
        hideMainWindow();
        return;
      }

      // History keyboard navigation when in "history" tab and not typing in a textarea
      const targetTag = (e.target as HTMLElement | null)?.tagName?.toLowerCase();
      if (activeTab === "history" && targetTag !== "textarea") {
        if (e.key === "ArrowDown" && clips.length > 0) {
          e.preventDefault();
          const currentIndex = clips.findIndex(
            (c) => c.id === selectedClip?.id
          );
          const nextIndex =
            currentIndex < clips.length - 1 ? currentIndex + 1 : 0;
          setSelectedClipId(clips[nextIndex].id);
        } else if (e.key === "ArrowUp" && clips.length > 0) {
          e.preventDefault();
          const currentIndex = clips.findIndex(
            (c) => c.id === selectedClip?.id
          );
          const prevIndex =
            currentIndex > 0 ? currentIndex - 1 : clips.length - 1;
          setSelectedClipId(clips[prevIndex].id);
        } else if (e.key === "Enter" && selectedClip) {
          e.preventDefault();
          handleCopyClip(selectedClip.id, e.shiftKey);
        } else if (
          (e.ctrlKey || e.metaKey) &&
          e.key.toLowerCase() === "p" &&
          selectedClip
        ) {
          e.preventDefault();
          handleTogglePin(selectedClip.id);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTab, clips, selectedClip, searchQuery, showNewClipModal]);

  useEffect(() => {
    expandTextPreview(snippetSandboxInput, false)
      .then(([expanded, triggers]) => {
        setSnippetSandboxOutput(expanded);
        setSnippetSandboxTriggers(triggers);
      })
      .catch(() => {});
  }, [snippetSandboxInput, snippets]);

  useEffect(() => {
    transformTextCmd(
      cleanerInput,
      optCleanUrls,
      optPrettifyJson,
      optMinifyJson,
      optCleanMarkdown,
      false
    )
      .then(setCleanerResult)
      .catch(() => {});
  }, [
    cleanerInput,
    optCleanUrls,
    optPrettifyJson,
    optMinifyJson,
    optCleanMarkdown,
  ]);

  const handleCopyClip = async (id: string, clean: boolean) => {
    try {
      const updated = await copyToOsClipboard(id, clean);
      await refreshHistory();
      setSelectedClipId(updated.id);
      showToast(
        clean
          ? "Copiado con limpieza automática aplicada"
          : "Copiado al portapapeles del sistema"
      );
    } catch (e: any) {
      showToast(`Error: ${e}`);
    }
  };

  const handleInPlaceTransform = async (
    id: string,
    urls: boolean,
    prettyJson: boolean,
    miniJson: boolean,
    md: boolean
  ) => {
    try {
      const updated = await applyCleanToItem(
        id,
        urls,
        prettyJson,
        miniJson,
        md
      );
      await refreshHistory();
      setSelectedClipId(updated.id);
      showToast("Formato limpiado y copiado al portapapeles");
    } catch (e: any) {
      showToast(`Error: ${e}`);
    }
  };

  const handleTogglePin = async (id: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    try {
      const pinned = await togglePinClip(id);
      await refreshHistory();
      showToast(pinned ? "Elemento fijado" : "Elemento desfijado");
    } catch (err: any) {
      showToast(`Error: ${err}`);
    }
  };

  const handleDeleteClip = async (id: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    try {
      await deleteClip(id);
      await refreshHistory();
      showToast("Elemento eliminado");
    } catch (err: any) {
      showToast(`Error: ${err}`);
    }
  };

  const handleCreateManualClip = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newClipText.trim()) return;
    try {
      const item = await addManualClip(newClipText, false, true);
      setNewClipText("");
      setShowNewClipModal(false);
      await refreshHistory();
      setSelectedClipId(item.id);
      showToast("Añadido y copiado al portapapeles");
    } catch (err: any) {
      showToast(`Error: ${err}`);
    }
  };

  const handleSaveSnippet = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await saveSnippetCmd(editingSnippet);
      setEditingSnippet({
        id: null,
        trigger: ";",
        title: "",
        body: "",
        category: "General",
        enabled: true,
      });
      await refreshSnippets();
      showToast("Snippet guardado correctamente");
    } catch (err: any) {
      showToast(`${err}`);
    }
  };

  const handleExpandSnippet = async (id: string) => {
    try {
      await expandAndCopySnippet(id);
      await refreshSnippets();
      await refreshHistory();
      showToast("Snippet expandido y copiado al portapapeles");
    } catch (err: any) {
      showToast(`Error: ${err}`);
    }
  };

  const handleUpdateSettings = async (patch: Partial<AppSettings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      await saveAppSettings(next);
      showToast("Ajustes y atajos globales actualizados");
    } catch (err: any) {
      showToast(`Error guardando ajustes: ${err}`);
    }
  };

  const handleSyncNow = async () => {
    try {
      const report = await syncNowCmd();
      setLastSyncReport(report);
      await refreshHistory();
      await refreshSnippets();
      showToast(
        `Sincronizado: +${report.imported_clips} clips, +${report.imported_snippets} snippets`
      );
    } catch (err: any) {
      showToast(`Error de sincronización: ${err}`);
    }
  };

  const handleSimulateAndroidSync = async () => {
    try {
      const report = await simulateRemoteDeviceSync();
      setLastSyncReport(report);
      await refreshHistory();
      await refreshSnippets();
      showToast(
        `Recibido desde Android-Pixel9: +${report.imported_clips} clips, +${report.imported_snippets} snippets`
      );
    } catch (err: any) {
      showToast(`Error simulando Android: ${err}`);
    }
  };

  const renderCategoryBadge = (cat: ClipCategory, lang?: string | null) => {
    switch (cat) {
      case "code":
        return (
          <span className="badge badge-code">
            <Code2 size={12} /> {lang ? lang.toUpperCase() : "CÓDIGO"}
          </span>
        );
      case "url":
        return (
          <span className="badge badge-url">
            <Link2 size={12} /> URL
          </span>
        );
      case "image":
        return (
          <span className="badge badge-image">
            <ImageIcon size={12} /> IMAGEN
          </span>
        );
      default:
        return (
          <span className="badge badge-text">
            <FileText size={12} /> TEXTO
          </span>
        );
    }
  };

  const formatRelativeTime = (ms: number) => {
    const diffSec = Math.max(0, Math.floor((Date.now() - ms) / 1000));
    if (diffSec < 60) return "Hace instantes";
    if (diffSec < 3600) return `Hace ${Math.floor(diffSec / 60)} min`;
    if (diffSec < 86400) return `Hace ${Math.floor(diffSec / 3600)} h`;
    return new Date(ms).toLocaleDateString("es-ES", {
      day: "2-digit",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  return (
    <div className="app-shell">
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-logo">
            <img src="/UltiCP.svg" alt="UltiCP" className="brand-logo-img" />
          </div>
          <div>
            <h1>UltiCP</h1>
            <span className="brand-sub">Local-First · Win + Android</span>
          </div>
        </div>

        <nav className="nav-menu">
          <button
            className={`nav-item ${activeTab === "history" ? "active" : ""}`}
            onClick={() => setActiveTab("history")}
          >
            <Clipboard size={17} />
            <span>Portapapeles</span>
            <span className="nav-counter">{clips.length}</span>
          </button>

          <button
            className={`nav-item ${activeTab === "snippets" ? "active" : ""}`}
            onClick={() => setActiveTab("snippets")}
          >
            <Zap size={17} />
            <span>Snippets</span>
            <span className="nav-counter">{snippets.length}</span>
          </button>

          <button
            className={`nav-item ${activeTab === "cleaner" ? "active" : ""}`}
            onClick={() => setActiveTab("cleaner")}
          >
            <Wand2 size={17} />
            <span>Limpiador Formato</span>
          </button>

          <button
            className={`nav-item ${activeTab === "sync" ? "active" : ""}`}
            onClick={() => setActiveTab("sync")}
          >
            <FolderSync size={17} />
            <span>Syncthing & Ajustes</span>
          </button>
        </nav>

        {settings && (
          <div className="sidebar-footer">
            <div className="quick-toggle-card">
              <div className="quick-toggle-row">
                <span>Invocar ventana</span>
                <kbd className="kbd-tag">{settings.shortcut_toggle_window}</kbd>
              </div>
              <div className="quick-toggle-row">
                <span>Limpiar al vuelo</span>
                <kbd className="kbd-tag">
                  {settings.shortcut_clean_clipboard.replace(
                    "CommandOrControl",
                    "Ctrl"
                  )}
                </kbd>
              </div>
              <div className="quick-toggle-row">
                <span>Escucha activa</span>
                <button
                  className={`pill-toggle ${
                    !settings.pause_monitoring ? "on" : "off"
                  }`}
                  onClick={() =>
                    handleUpdateSettings({
                      pause_monitoring: !settings.pause_monitoring,
                    })
                  }
                >
                  {settings.pause_monitoring ? (
                    <>
                      <PauseCircle size={13} /> Pausado
                    </>
                  ) : (
                    <>
                      <PlayCircle size={13} /> Activo
                    </>
                  )}
                </button>
              </div>
              <div className="quick-toggle-row">
                <span>Auto-limpiar URLs</span>
                <input
                  type="checkbox"
                  checked={settings.auto_clean_urls}
                  onChange={(e) =>
                    handleUpdateSettings({ auto_clean_urls: e.target.checked })
                  }
                />
              </div>
              <div className="quick-toggle-row">
                <span>Auto-expandir Snippets</span>
                <input
                  type="checkbox"
                  checked={settings.snippet_expansion_enabled}
                  onChange={(e) =>
                    handleUpdateSettings({
                      snippet_expansion_enabled: e.target.checked,
                    })
                  }
                />
              </div>
            </div>
          </div>
        )}
      </aside>

      {/* Main Content Area */}
      <main className="main-panel">
        {/* TAB 1: CLIPBOARD HISTORY */}
        {activeTab === "history" && (
          <div className="tab-container">
            <header className="top-bar">
              <div className="search-box">
                <Search size={16} className="search-icon" />
                <input
                  ref={searchInputRef}
                  type="text"
                  placeholder="Buscar (Ctrl+K) · ↑/↓ navegar · Enter copiar · Shift+Enter copiar limpio · Esc ocultar"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                />
              </div>

              <div className="top-actions">
                <button
                  className="btn btn-primary"
                  onClick={() => setShowNewClipModal(true)}
                >
                  <Plus size={15} /> Nuevo Clip
                </button>
                <button
                  className="btn btn-ghost"
                  onClick={handleSyncNow}
                  title="Sincronizar con carpeta Syncthing"
                >
                  <RefreshCw size={15} /> Sync
                </button>
                <button
                  className="btn btn-danger-ghost"
                  onClick={async () => {
                    const count = await clearUnpinnedClips();
                    await refreshHistory();
                    showToast(`Eliminados ${count} elementos no fijados`);
                  }}
                  title="Limpiar historial no fijado"
                >
                  <Trash2 size={15} />
                </button>
              </div>
            </header>

            {/* Category Filter Bar */}
            <div className="filter-bar">
              <div className="category-pills">
                {[
                  { id: "all", label: "Todos" },
                  { id: "plain_text", label: "Texto plano" },
                  { id: "code", label: "Código" },
                  { id: "url", label: "URLs" },
                  { id: "image", label: "Imágenes" },
                ].map((cat) => (
                  <button
                    key={cat.id}
                    className={`cat-pill ${
                      categoryFilter === cat.id ? "active" : ""
                    }`}
                    onClick={() => setCategoryFilter(cat.id)}
                  >
                    {cat.label}
                  </button>
                ))}
              </div>

              <button
                className={`cat-pill pin-filter ${pinnedOnly ? "active" : ""}`}
                onClick={() => setPinnedOnly(!pinnedOnly)}
              >
                <Pin size={13} /> Fijados
              </button>
            </div>

            {showNewClipModal && (
              <form className="new-clip-bar" onSubmit={handleCreateManualClip}>
                <textarea
                  rows={3}
                  placeholder="Pega o escribe un texto, JSON minificado, Markdown o URL con tracking para añadirlo al portapapeles..."
                  value={newClipText}
                  onChange={(e) => setNewClipText(e.target.value)}
                  autoFocus
                />
                <div className="new-clip-actions">
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => setShowNewClipModal(false)}
                  >
                    Cancelar
                  </button>
                  <button type="submit" className="btn btn-primary">
                    Guardar y Copiar
                  </button>
                </div>
              </form>
            )}

            {/* Split View: List + Inspector */}
            <div className="history-split">
              <div className="clip-list">
                {clips.length === 0 ? (
                  <div className="empty-state">
                    <Clipboard size={34} />
                    <p>No hay elementos en esta categoría.</p>
                    <span>
                      Copia cualquier texto, URL, código o imagen en Windows
                      (`Ctrl+C`) y aparecerá aquí al instante.
                    </span>
                  </div>
                ) : (
                  clips.map((item) => (
                    <div
                      key={item.id}
                      className={`clip-card ${
                        selectedClip?.id === item.id ? "selected" : ""
                      }`}
                      onClick={() => setSelectedClipId(item.id)}
                      onDoubleClick={() => handleCopyClip(item.id, false)}
                    >
                      <div className="clip-card-header">
                        {renderCategoryBadge(item.category, item.language)}
                        <span className="clip-device">
                          {item.source_device
                            .toLowerCase()
                            .includes("android") ? (
                            <Smartphone size={12} />
                          ) : null}
                          {item.source_device}
                        </span>
                        <span className="clip-time">
                          {formatRelativeTime(item.updated_at)}
                        </span>
                      </div>

                      {item.category === "image" && item.image_data_url ? (
                        <div className="clip-image-thumb">
                          <img src={item.image_data_url} alt={item.preview} />
                        </div>
                      ) : (
                        <p className="clip-preview">{item.preview}</p>
                      )}

                      <div className="clip-card-footer">
                        <span className="clip-meta">
                          {item.byte_size} B · Copiado {item.copy_count}x
                        </span>
                        <div className="clip-card-buttons">
                          <button
                            className="icon-btn"
                            title="Copiar al portapapeles (Enter)"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleCopyClip(item.id, false);
                            }}
                          >
                            <Copy size={14} />
                          </button>
                          <button
                            className="icon-btn"
                            title="Pegar limpio (Shift+Enter)"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleCopyClip(item.id, true);
                            }}
                          >
                            <Sparkles size={14} />
                          </button>
                          <button
                            className={`icon-btn ${
                              item.is_pinned ? "pinned" : ""
                            }`}
                            title="Fijar elemento (Ctrl+P)"
                            onClick={(e) => handleTogglePin(item.id, e)}
                          >
                            <Pin size={14} />
                          </button>
                          <button
                            className="icon-btn danger"
                            title="Eliminar"
                            onClick={(e) => handleDeleteClip(item.id, e)}
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>

              {/* Detail Inspector */}
              <div className="clip-inspector">
                {selectedClip ? (
                  <>
                    <div className="inspector-header">
                      <div className="inspector-title">
                        {renderCategoryBadge(
                          selectedClip.category,
                          selectedClip.language
                        )}
                        <span className="inspector-device">
                          Origen: <strong>{selectedClip.source_device}</strong>
                        </span>
                      </div>
                      <div className="inspector-actions">
                        <button
                          className="btn btn-primary"
                          onClick={() => handleCopyClip(selectedClip.id, false)}
                        >
                          <Copy size={15} /> Copiar <kbd className="kbd-inline">Enter</kbd>
                        </button>
                        <button
                          className="btn btn-accent"
                          onClick={() => handleCopyClip(selectedClip.id, true)}
                        >
                          <Sparkles size={15} /> Copiar Limpio{" "}
                          <kbd className="kbd-inline">Shift+Enter</kbd>
                        </button>
                      </div>
                    </div>

                    {selectedClip.category !== "image" && (
                      <div className="inspector-clean-toolbar">
                        <span>Transformar in-situ:</span>
                        <button
                          className="chip-btn"
                          onClick={() =>
                            handleInPlaceTransform(
                              selectedClip.id,
                              true,
                              false,
                              false,
                              false
                            )
                          }
                        >
                          <Link2 size={13} /> Limpiar Tracking URL
                        </button>
                        <button
                          className="chip-btn"
                          onClick={() =>
                            handleInPlaceTransform(
                              selectedClip.id,
                              false,
                              true,
                              false,
                              false
                            )
                          }
                        >
                          <Code2 size={13} /> JSON Legible
                        </button>
                        <button
                          className="chip-btn"
                          onClick={() =>
                            handleInPlaceTransform(
                              selectedClip.id,
                              false,
                              false,
                              true,
                              false
                            )
                          }
                        >
                          <Code2 size={13} /> Minificar JSON
                        </button>
                        <button
                          className="chip-btn"
                          onClick={() =>
                            handleInPlaceTransform(
                              selectedClip.id,
                              true,
                              false,
                              false,
                              true
                            )
                          }
                        >
                          <FileText size={13} /> Formatear Markdown
                        </button>
                      </div>
                    )}

                    <div className="inspector-body">
                      {selectedClip.category === "image" &&
                      selectedClip.image_data_url ? (
                        <div className="inspector-image-box">
                          <img
                            src={selectedClip.image_data_url}
                            alt={selectedClip.preview}
                          />
                        </div>
                      ) : (
                        <pre className="inspector-code">
                          {selectedClip.content}
                        </pre>
                      )}
                    </div>

                    <div className="inspector-footer">
                      <span>Tamaño: {selectedClip.byte_size} bytes</span>
                      <span>
                        SHA-256: {selectedClip.content_hash.slice(0, 16)}…
                      </span>
                      <span>Veces copiado: {selectedClip.copy_count}</span>
                    </div>
                  </>
                ) : (
                  <div className="empty-state">
                    <p>Selecciona un elemento para inspeccionarlo</p>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* TAB 2: SNIPPETS */}
        {activeTab === "snippets" && (
          <div className="tab-container">
            <div className="section-header">
              <div>
                <h2>Expansión de Snippets y Plantillas</h2>
                <p>
                  Define abreviaturas (ej. <code>;firma</code>,{" "}
                  <code>;reunion</code>, <code>;pr</code>) con variables
                  dinámicas. Al copiarlas o usar el expansor se sustituyen
                  automáticamente.
                </p>
              </div>
            </div>

            <div className="snippets-grid">
              {/* Left: Snippets List + Live Expander */}
              <div className="snippets-list-col">
                <div className="card-panel">
                  <h3>Probador / Expansor en Vivo</h3>
                  <textarea
                    rows={3}
                    value={snippetSandboxInput}
                    onChange={(e) => setSnippetSandboxInput(e.target.value)}
                    placeholder="Escribe texto con abreviaturas como ;firma o ;fecha..."
                  />
                  {snippetSandboxTriggers.length > 0 && (
                    <div className="detected-triggers">
                      Triggers detectados:{" "}
                      {snippetSandboxTriggers.map((t) => (
                        <span key={t} className="badge badge-code">
                          {t}
                        </span>
                      ))}
                    </div>
                  )}
                  <pre className="sandbox-preview">{snippetSandboxOutput}</pre>
                  <button
                    className="btn btn-primary"
                    onClick={async () => {
                      await expandTextPreview(snippetSandboxInput, true);
                      showToast("Texto expandido copiado al portapapeles");
                    }}
                  >
                    <Copy size={14} /> Copiar Resultado Expandido
                  </button>
                </div>

                <div className="snippets-items">
                  {snippets.map((s) => (
                    <div key={s.id} className="snippet-card">
                      <div className="snippet-card-top">
                        <span className="snippet-trigger">{s.trigger}</span>
                        <strong className="snippet-title">{s.title}</strong>
                        <span className="badge badge-text">{s.category}</span>
                      </div>
                      <pre className="snippet-body-preview">{s.body}</pre>
                      <div className="snippet-card-actions">
                        <span className="clip-meta">Usado {s.use_count}x</span>
                        <div className="btn-row">
                          <button
                            className="btn btn-primary btn-sm"
                            onClick={() => handleExpandSnippet(s.id)}
                          >
                            <Zap size={13} /> Expandir y Copiar
                          </button>
                          <button
                            className="btn btn-ghost btn-sm"
                            onClick={() =>
                              setEditingSnippet({
                                id: s.id,
                                trigger: s.trigger,
                                title: s.title,
                                body: s.body,
                                category: s.category,
                                enabled: s.enabled,
                              })
                            }
                          >
                            Editar
                          </button>
                          <button
                            className="icon-btn danger"
                            onClick={async () => {
                              await deleteSnippetCmd(s.id);
                              await refreshSnippets();
                              showToast("Snippet eliminado");
                            }}
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              {/* Right: Snippet Editor */}
              <form
                className="card-panel snippet-form"
                onSubmit={handleSaveSnippet}
              >
                <h3>
                  {editingSnippet.id ? "Editar Snippet" : "Nuevo Snippet"}
                </h3>

                <label>
                  Abreviatura (Trigger)
                  <input
                    type="text"
                    required
                    placeholder=";miatajo"
                    value={editingSnippet.trigger}
                    onChange={(e) =>
                      setEditingSnippet({
                        ...editingSnippet,
                        trigger: e.target.value,
                      })
                    }
                  />
                </label>

                <label>
                  Título descriptivo
                  <input
                    type="text"
                    required
                    placeholder="Ej. Respuesta soporte técnico"
                    value={editingSnippet.title}
                    onChange={(e) =>
                      setEditingSnippet({
                        ...editingSnippet,
                        title: e.target.value,
                      })
                    }
                  />
                </label>

                <label>
                  Categoría
                  <input
                    type="text"
                    placeholder="Trabajo, Código, Personal..."
                    value={editingSnippet.category}
                    onChange={(e) =>
                      setEditingSnippet({
                        ...editingSnippet,
                        category: e.target.value,
                      })
                    }
                  />
                </label>

                <div className="placeholder-bar">
                  <span>Insertar variable:</span>
                  {[
                    "{{date}}",
                    "{{time}}",
                    "{{datetime}}",
                    "{{clipboard}}",
                    "{{uuid}}",
                  ].map((ph) => (
                    <button
                      type="button"
                      key={ph}
                      className="chip-btn"
                      onClick={() =>
                        setEditingSnippet({
                          ...editingSnippet,
                          body: editingSnippet.body + ph,
                        })
                      }
                    >
                      {ph}
                    </button>
                  ))}
                </div>

                <label>
                  Plantilla / Contenido a expandir
                  <textarea
                    rows={7}
                    required
                    placeholder="Escribe el contenido completo de la plantilla..."
                    value={editingSnippet.body}
                    onChange={(e) =>
                      setEditingSnippet({
                        ...editingSnippet,
                        body: e.target.value,
                      })
                    }
                  />
                </label>

                <div className="btn-row">
                  {editingSnippet.id && (
                    <button
                      type="button"
                      className="btn btn-ghost"
                      onClick={() =>
                        setEditingSnippet({
                          id: null,
                          trigger: ";",
                          title: "",
                          body: "",
                          category: "General",
                          enabled: true,
                        })
                      }
                    >
                      Cancelar edición
                    </button>
                  )}
                  <button type="submit" className="btn btn-primary">
                    <Check size={15} /> Guardar Snippet
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}

        {/* TAB 3: CLEANER / FORMATTER LAB */}
        {activeTab === "cleaner" && (
          <div className="tab-container">
            <div className="section-header">
              <div>
                <h2>Limpieza Automática de Formato al Pegar</h2>
                <p>
                  Elimina parámetros de seguimiento (<code>utm_*</code>,{" "}
                  <code>fbclid</code>, <code>gclid</code>, <code>si</code>),
                  convierte JSON minificado en legible y normaliza Markdown.
                </p>
              </div>
            </div>

            <div className="cleaner-controls card-panel">
              <label className="check-label">
                <input
                  type="checkbox"
                  checked={optCleanUrls}
                  onChange={(e) => setOptCleanUrls(e.target.checked)}
                />
                Eliminar tracking URLs (utm_*, fbclid, gclid, igshid, si...)
              </label>

              <label className="check-label">
                <input
                  type="checkbox"
                  checked={optPrettifyJson}
                  onChange={(e) => {
                    setOptPrettifyJson(e.target.checked);
                    if (e.target.checked) setOptMinifyJson(false);
                  }}
                />
                Convertir JSON minificado a legible (Prettify)
              </label>

              <label className="check-label">
                <input
                  type="checkbox"
                  checked={optMinifyJson}
                  onChange={(e) => {
                    setOptMinifyJson(e.target.checked);
                    if (e.target.checked) setOptPrettifyJson(false);
                  }}
                />
                Minificar JSON
              </label>

              <label className="check-label">
                <input
                  type="checkbox"
                  checked={optCleanMarkdown}
                  onChange={(e) => setOptCleanMarkdown(e.target.checked)}
                />
                Formatear y limpiar Markdown
              </label>
            </div>

            <div className="cleaner-split">
              <div className="card-panel">
                <h3>Entrada Original</h3>
                <textarea
                  rows={14}
                  value={cleanerInput}
                  onChange={(e) => setCleanerInput(e.target.value)}
                  placeholder="Pega aquí una URL con tracking, JSON minificado o texto Markdown desordenado..."
                />
              </div>

              <div className="card-panel">
                <div className="cleaner-out-header">
                  <h3>Resultado Limpio</h3>
                  <button
                    className="btn btn-primary btn-sm"
                    onClick={async () => {
                      await transformTextCmd(
                        cleanerInput,
                        optCleanUrls,
                        optPrettifyJson,
                        optMinifyJson,
                        optCleanMarkdown,
                        true
                      );
                      await refreshHistory();
                      showToast("Resultado limpio copiado al portapapeles");
                    }}
                  >
                    <Copy size={14} /> Copiar Limpio
                  </button>
                </div>

                {cleanerResult && cleanerResult.applied_actions.length > 0 && (
                  <div className="applied-badges">
                    {cleanerResult.applied_actions.map((act, idx) => (
                      <span key={idx} className="badge badge-url">
                        <Check size={12} /> {act}
                      </span>
                    ))}
                  </div>
                )}

                <pre className="cleaner-output">
                  {cleanerResult?.result ?? cleanerInput}
                </pre>
              </div>
            </div>
          </div>
        )}

        {/* TAB 4: SYNCTHING, GLOBAL SHORTCUTS & SETTINGS */}
        {activeTab === "sync" && settings && (
          <div className="tab-container">
            <div className="section-header">
              <div>
                <h2>Sincronización Syncthing & Atajos Globales</h2>
                <p>
                  Configura los atajos de teclado globales en todo Windows y la
                  sincronización P2P con Android.
                </p>
              </div>
            </div>

            <div className="sync-grid">
              {/* Global Shortcuts Config */}
              <div className="card-panel">
                <h3>
                  <Keyboard size={18} /> Atajos de Teclado Globales (Sistema)
                </h3>
                <p className="helper-text">
                  Funcionan en segundo plano desde cualquier aplicación de
                  Windows. Al cerrar la ventana, UltiCP permanece activo en la
                  bandeja del sistema (System Tray).
                </p>

                <label>
                  Mostrar / Ocultar UltiCP y enfocar buscador
                  <input
                    type="text"
                    value={settings.shortcut_toggle_window}
                    onChange={(e) =>
                      handleUpdateSettings({
                        shortcut_toggle_window: e.target.value,
                      })
                    }
                    placeholder="Alt+Space"
                  />
                </label>

                <label>
                  Limpiar y expandir portapapeles actual al vuelo (sin abrir
                  ventana)
                  <input
                    type="text"
                    value={settings.shortcut_clean_clipboard}
                    onChange={(e) =>
                      handleUpdateSettings({
                        shortcut_clean_clipboard: e.target.value,
                      })
                    }
                    placeholder="CommandOrControl+Shift+C"
                  />
                </label>

                <label>
                  Abrir directamente pestaña de Snippets
                  <input
                    type="text"
                    value={settings.shortcut_open_snippets}
                    onChange={(e) =>
                      handleUpdateSettings({
                        shortcut_open_snippets: e.target.value,
                      })
                    }
                    placeholder="CommandOrControl+Shift+S"
                  />
                </label>

                <div className="shortcuts-cheatsheet">
                  <strong>Atajos rápidos dentro de la ventana:</strong>
                  <div className="cheatsheet-grid">
                    <span>
                      <kbd className="kbd-tag">↑</kbd> /{" "}
                      <kbd className="kbd-tag">↓</kbd> Navegar historial
                    </span>
                    <span>
                      <kbd className="kbd-tag">Enter</kbd> Copiar selección
                    </span>
                    <span>
                      <kbd className="kbd-tag">Shift+Enter</kbd> Copiar limpio
                    </span>
                    <span>
                      <kbd className="kbd-tag">Ctrl+P</kbd> Fijar / Desfijar
                    </span>
                    <span>
                      <kbd className="kbd-tag">Ctrl+K</kbd> Buscar
                    </span>
                    <span>
                      <kbd className="kbd-tag">Esc</kbd> Ocultar a bandeja
                    </span>
                  </div>
                </div>
              </div>

              {/* Syncthing & Auto Rules */}
              <div className="card-panel">
                <h3>
                  <FolderSync size={18} /> Carpeta Syncthing (Windows ↔ Android)
                </h3>
                <label>
                  Nombre de este dispositivo
                  <input
                    type="text"
                    value={settings.device_name}
                    onChange={(e) =>
                      handleUpdateSettings({ device_name: e.target.value })
                    }
                  />
                </label>

                <label>
                  Ruta de carpeta compartida en Syncthing
                  <input
                    type="text"
                    value={settings.sync_folder}
                    onChange={(e) =>
                      handleUpdateSettings({ sync_folder: e.target.value })
                    }
                    placeholder="C:\Users\tu_usuario\Sync\UltiCP"
                  />
                </label>

                <div className="btn-row">
                  <button className="btn btn-primary" onClick={handleSyncNow}>
                    <RefreshCw size={15} /> Sincronizar Ahora
                  </button>
                  <button
                    className="btn btn-accent"
                    onClick={handleSimulateAndroidSync}
                  >
                    <Smartphone size={15} /> Simular Recepción desde Android
                  </button>
                </div>

                {lastSyncReport && (
                  <div className="sync-report-box">
                    <strong>Última sincronización exitosa:</strong>
                    <p>
                      Exportados: {lastSyncReport.exported_clips} clips,{" "}
                      {lastSyncReport.exported_snippets} snippets · Importados:{" "}
                      {lastSyncReport.imported_clips} clips,{" "}
                      {lastSyncReport.imported_snippets} snippets
                    </p>
                  </div>
                )}

                <hr className="divider" />

                <h3>
                  <Settings size={18} /> Reglas Automáticas al Copiar
                </h3>

                <label className="check-label">
                  <input
                    type="checkbox"
                    checked={settings.auto_clean_urls}
                    onChange={(e) =>
                      handleUpdateSettings({ auto_clean_urls: e.target.checked })
                    }
                  />
                  Limpiar automáticamente tracking URLs (utm_*, fbclid, gclid)
                </label>

                <label className="check-label">
                  <input
                    type="checkbox"
                    checked={settings.auto_format_json}
                    onChange={(e) =>
                      handleUpdateSettings({
                        auto_format_json: e.target.checked,
                      })
                    }
                  />
                  Formatear automáticamente JSON minificado al copiar
                </label>

                <label className="check-label">
                  <input
                    type="checkbox"
                    checked={settings.auto_format_markdown}
                    onChange={(e) =>
                      handleUpdateSettings({
                        auto_format_markdown: e.target.checked,
                      })
                    }
                  />
                  Normalizar formato Markdown automáticamente al copiar
                </label>

                <label className="check-label">
                  <input
                    type="checkbox"
                    checked={settings.snippet_expansion_enabled}
                    onChange={(e) =>
                      handleUpdateSettings({
                        snippet_expansion_enabled: e.target.checked,
                      })
                    }
                  />
                  Expandir abreviaturas de Snippets automáticamente al copiarlas
                </label>
              </div>
            </div>
          </div>
        )}
      </main>

      {toast && <div className="toast-banner">{toast}</div>}
    </div>
  );
}
