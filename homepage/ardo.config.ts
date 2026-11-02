import { defineArdoConfig } from "ardo/config";

export default defineArdoConfig({
  title: "Ferrolink",
  description:
    "Find broken links, missing assets and invalid anchors in your finished static build. A native checker for the CLI, Node.js and Vite, without a server or network.",
  base: "/ferrolink/",
  siteUrl: "https://sebastian-software.github.io",
  icons: false,
  markdown: { headingAttributes: true },
  linkCheck: { level: "error" },
});
