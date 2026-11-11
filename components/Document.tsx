import type { ReactNode } from "react";
import type { Language } from "@/lib/pages";
import "@/assets/style.css";
import "@/assets/miners.css";
import "@/assets/api.css";
import "@/assets/pool-theme.css";
import "@/assets/pages.css";
import "@/assets/pulse-theme.css";

// This small synchronous script applies the saved theme before first paint.
// The React theme button takes over after hydration.
const initialTheme = `try{document.documentElement.dataset.theme=localStorage.getItem('bdl-pool:theme')==='light'?'light':'dark'}catch{document.documentElement.dataset.theme='dark'}`;

export function Document({
  lang,
  children,
}: {
  lang: Language;
  children: ReactNode;
}) {
  return (
    <html lang={lang} suppressHydrationWarning>
      <head>
        <meta name="color-scheme" content="light dark" />
        <meta name="theme-color" content="#141415" />
        <script dangerouslySetInnerHTML={{ __html: initialTheme }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
