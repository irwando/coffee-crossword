import { useEffect, useRef } from "react";

interface NumberPromptDialogProps {
  open: boolean;
  title: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onSave: (value: number) => void;
  onCancel: () => void;
}

// Small numeric-input modal for menu-triggered settings (Max Results, Timeout)
// that don't fit a checkbox. Styled to match DictionaryPanel/ExternalLookupPanel's
// centered-backdrop pattern.
export default function NumberPromptDialog({
  open, title, label, value, min, max, step = 1, unit, onSave, onCancel,
}: NumberPromptDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      const id = requestAnimationFrame(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      });
      return () => cancelAnimationFrame(id);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onCancel(); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onCancel]);

  if (!open) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const raw = Number(inputRef.current?.value);
    const clamped = Math.min(max, Math.max(min, Number.isFinite(raw) ? raw : value));
    onSave(clamped);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 dark:bg-black/60"
      onMouseDown={onCancel}
    >
      <div
        className="relative bg-white dark:bg-gray-900 rounded-xl shadow-2xl border border-gray-200 dark:border-gray-700 w-full max-w-xs mx-4"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <form onSubmit={handleSubmit} noValidate>
          <div className="px-5 pt-4 pb-3">
            <h2 className="text-sm font-bold text-gray-900 dark:text-gray-100 mb-3">{title}</h2>
            <label className="block text-xs text-gray-500 dark:text-gray-400 mb-1">{label}</label>
            <div className="flex items-center gap-2">
              <input
                ref={inputRef}
                type="number"
                defaultValue={value}
                min={min}
                max={max}
                step={step}
                className="w-full px-2 py-1 border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-800 dark:text-gray-100 bg-white dark:bg-gray-800"
              />
              {unit && <span className="text-xs text-gray-400">{unit}</span>}
            </div>
          </div>
          <div className="flex justify-end gap-2 px-5 py-3 border-t border-gray-100 dark:border-gray-800">
            <button
              type="button"
              onClick={onCancel}
              className="px-3 py-1 text-xs rounded text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="px-3 py-1 text-xs rounded bg-blue-500 text-white hover:bg-blue-600"
            >
              Save
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
