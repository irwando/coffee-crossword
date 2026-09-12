// ── Reference panel data ──────────────────────────────────────────────────────

export const REFERENCE_ROWS = [
  { feature: "Template",         pattern: ".l...r.n",    match: "electron",      note: ". or ? = any letter"        },
  { feature: "Anagram",          pattern: ";acenrt",      match: "canter",        note: "; prefix = rearrange"        },
  { feature: "Wildcard",         pattern: "m*ja",         match: "maharaja",      note: "* = zero or more letters"   },
  { feature: "Choice list",      pattern: "[aeiou]....",  match: "ultra",         note: "any one letter from set"    },
  { feature: "Negated choice",   pattern: "[^aeiou]...",  match: "cast",          note: "any letter not in set"      },
  { feature: "Macro",            pattern: "@....",        match: "ultra",         note: "@ = vowel, # = consonant"   },
  { feature: "Anagram blank",    pattern: ";acenrt.",     match: "cantered +ED",  note: ". = one unknown letter"     },
  { feature: "Anagram wildcard", pattern: ";cats*",       match: "escalator",     note: "* = any extra letters"      },
  { feature: "Tmpl + anagram",   pattern: "e.....;cats",  match: "enacts",        note: "combine both styles"        },
  { feature: "Letter variable",  pattern: "12321",        match: "level",         note: "same digit = same letter"   },
  { feature: "AND",              pattern: "c* & *s",      match: "cats",          note: "must match both"            },
  { feature: "OR",               pattern: "c... | ...r",  match: "cast",          note: "matches either"             },
  { feature: "NOT",              pattern: "c* & !cat*",   match: "cast",          note: "exclude matches"            },
  { feature: "Sub-pattern",      pattern: "...(;orange)", match: "patronage",     note: "() switches mode"           },
  { feature: "Punctuation",      pattern: "...-..-....", match: "pick-me-up",    note: "normalize off to use"       },
  { feature: "Exact length",     pattern: "5:cat*",       match: "catty",         note: "X: = exactly X letters"     },
  { feature: "Min length",       pattern: "8-:cat*",      match: "category",      note: "X-: = at least X letters"   },
  { feature: "Max length",       pattern: "-4:cat*",      match: "cats",          note: "-X: = at most X letters"    },
  { feature: "Length range",     pattern: "5-6:cat*",     match: "catchy",        note: "X-Y: = X to Y letters"      },
];

interface HeaderAction {
  label: string;
  onClick: () => void;
}

// ── Reference panels ──────────────────────────────────────────────────────────

export function ReferenceHeader({ action }: { action?: HeaderAction }) {
  return (
    <div className="flex items-center justify-between px-3 mb-1.5">
      <span className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide">
        Pattern reference
      </span>
      {action && (
        <button
          onClick={action.onClick}
          className="text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 normal-case font-normal"
        >
          {action.label}
        </button>
      )}
    </div>
  );
}

export function ReferenceFull({
  onPatternClick, headerAction,
}: { onPatternClick: (p: string) => void; headerAction?: HeaderAction }) {
  return (
    <div className="mb-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <div className="px-3 pt-2 pb-1"><ReferenceHeader action={headerAction} /></div>
      <table className="w-full text-xs" style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr className="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Feature</th>
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Pattern</th>
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Match</th>
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Notes</th>
          </tr>
        </thead>
        <tbody>
          {REFERENCE_ROWS.map((row) => (
            <tr key={row.feature}
              className="border-b border-gray-100 dark:border-gray-700 last:border-0 hover:bg-white dark:hover:bg-gray-700 cursor-pointer"
              onClick={() => onPatternClick(row.pattern)}
            >
              <td className="px-3 py-1.5 text-gray-600 dark:text-gray-300 font-medium">{row.feature}</td>
              <td className="px-3 py-1.5">
                <span className="font-mono text-gray-800 dark:text-gray-200 bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded px-1.5 py-0.5">
                  {row.pattern}
                </span>
              </td>
              <td className="px-3 py-1.5 font-mono text-gray-500 dark:text-gray-400">{row.match}</td>
              <td className="px-3 py-1.5 text-gray-400 dark:text-gray-500">{row.note}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function ReferenceCompact({
  onPatternClick, singleColumn = false, headerAction,
}: { onPatternClick: (p: string) => void; singleColumn?: boolean; headerAction?: HeaderAction }) {
  return (
    <div className="mb-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2">
      <ReferenceHeader action={headerAction} />
      <div className={`grid ${singleColumn ? "grid-cols-1" : "grid-cols-2"} gap-x-4 gap-y-0.5`}>
        {REFERENCE_ROWS.map((row) => (
          <div
            key={row.feature}
            onClick={() => onPatternClick(row.pattern)}
            className="flex items-baseline gap-1 font-mono text-xs overflow-hidden cursor-pointer hover:opacity-70"
          >
            <span className="font-sans text-gray-500 dark:text-gray-400 flex-shrink-0 text-xs">{row.feature}</span>
            <span className="text-gray-300 dark:text-gray-600 flex-shrink-0">(</span>
            <span className="text-gray-800 dark:text-gray-200 flex-shrink-0">{row.pattern}</span>
            <span className="text-gray-400 dark:text-gray-500 flex-shrink-0">→</span>
            <span className="text-gray-500 dark:text-gray-400 truncate">{row.match.split(",")[0].split(" ")[0]}</span>
            <span className="text-gray-300 dark:text-gray-600 flex-shrink-0">)</span>
          </div>
        ))}
      </div>
    </div>
  );
}
