// The Prism outline icon set (24x24, stroke 1.75, round caps, currentColor), shared with PrismQDA's style (doc 09).

const P: Record<string, string> = {
 home:"M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z",
 data:"M4 5h16v14H4z M4 10h16 M4 15h16 M10 5v14",
 output:"M6 3h9l4 4v14H6z M9.5 17v-3 M12.5 17v-6 M15.5 17v-4",
 syntax:"M8 8l-4 4 4 4 M16 8l4 4-4 4 M13.5 6l-3 12",
 report:"M6 3h9l4 4v14H6z M9 12h7 M9 16h7 M9 8h3",
 learn:"M2 9l10-5 10 5-10 5z M6 11v5c3 2 9 2 12 0v-5",
 settings:"M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
 plus:"M12 5v14 M5 12h14",
 folder:"M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
 opendata:"M14 3H6v18h12V7z M14 3v4h4 M9 13l3-3 3 3 M12 10v7",
 undo:"M9 14 4 9l5-5 M4 9h10a6 6 0 0 1 0 12h-3",
 redo:"M15 14l5-5-5-5 M20 9H10a6 6 0 0 0 0 12h3",
 save:"M5 3h11l3 3v15H5z M8 3v5h7V3 M8 21v-7h8v7",
 run:"M7 4v16l13-8z",
 bulb:"M9 18h6 M10 21h4 M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3z",
 info:"M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M12 11v5 M12 7.5v.5",
 search:"M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14z M21 21l-5-5",
 vars:"M9 6h11 M9 12h11 M9 18h11 M4.5 6h.01 M4.5 12h.01 M4.5 18h.01",
 history:"M3 12a9 9 0 1 0 3-6.7L3 8 M3 3v5h5 M12 7v5l3 2",
 close:"M6 6l12 12 M18 6 6 18",
 right:"M5 12h14 M13 6l6 6-6 6", left:"M19 12H5 M11 6l-6 6 6 6",
 chev:"M9 6l6 6-6 6", down:"M6 9l6 6 6-6",
 check:"M5 12l5 5 9-10",
 warn:"M12 3l9.5 17h-19z M12 10v4 M12 17v.5",
 minus:"M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M8 12h8",
 scale:"M3.5 16.5l13-13 4 4-13 13z M7.5 12.5l2 2 M10.5 9.5l2 2 M13.5 6.5l2 2",
 ordinal:"M5 19v-4 M10 19v-8 M15 19V7 M20 19V4",
 nominal:"M5 5h5v5H5z M14 5h5v5h-5z M9.5 14h5v5h-5z",
 idv:"M3 7h18v10H3z M7 11h.01 M10 11h7 M7 14h10",
 date:"M4 6h16v14H4z M4 10h16 M8 3v5 M16 3v5",
 text:"M5 6h14 M12 6v13 M9 19h6",
 chart:"M4 20V4 M4 20h16 M8 16v-4 M12 16V8 M16 16v-6",
 export:"M14 3H6v18h12V7z M14 3v4h4 M12 11v6 M9 14l3 3 3-3",
 copy:"M9 9h11v11H9z M5 15H4V4h11v1",
 compare:"M8 3v18 M16 3v18 M3 8h5 M16 16h5",
 refresh:"M20 11a8 8 0 0 0-14.9-3 M4 4v4h4 M4 13a8 8 0 0 0 14.9 3 M20 20v-4h-4",
 users:"M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z M2 21v-1a6 6 0 0 1 12 0v1 M16 3.5a4 4 0 0 1 0 7 M18 14a6 6 0 0 1 4 6v1",
 file:"M14 3H6v18h12V7z M14 3v4h4",
 transform:"M4 7h12 M13 4l3 3-3 3 M20 17H8 M11 14l-3 3 3 3",
 bold:"M7 5h6a3.5 3.5 0 0 1 0 7H7z M7 12h7a3.5 3.5 0 0 1 0 7H7z",
 link:"M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1 M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
 pen:"M4 20h4L19 9l-4-4L4 16z M14 6l4 4",
 sparkle:"M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"
};

export type IconName = keyof typeof P;

export function Icon({ name, className = '' }: { name: string; className?: string }) {
  return (
    <svg className={`i ${className}`} viewBox="0 0 24 24" aria-hidden="true">
      <path d={P[name] ?? ''} />
    </svg>
  );
}
