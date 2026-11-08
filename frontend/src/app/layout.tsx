import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Cull Workbench — Clean, Unify, Lint, Load",
  description: "High-speed tabular and document ETL auditing and schema migration workbench",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
