export const documents = [
  {
    source: "homepage/content/getting-started.md",
    path: "guide/getting-started",
    title: "Getting started",
    section: "guide",
  },
  { source: "README.md.src", path: "reference/cli", title: "CLI and checks", section: "reference" },
  {
    source: "node/README.md",
    path: "reference/node",
    title: "Node.js and Vite",
    section: "reference",
  },
  {
    source: "docs/parsers.md",
    path: "evidence/parsers",
    title: "Choosing the HTML parser",
    section: "evidence",
  },
  { source: "CHANGELOG.md", path: "changelog", title: "Changelog", section: "changelog" },
];
export const expectedPages = ["index.html", ...documents.map(({ path }) => `${path}/index.html`)];
export function pagePath(page) {
  return `/${page === "index.html" ? "" : page.replace(/index\.html$/, "")}`;
}
