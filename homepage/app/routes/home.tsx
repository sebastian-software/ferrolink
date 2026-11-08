import type { ArdoMetaFunction } from "ardo/runtime";

import { Fragment } from "ardo/react";
import { ArdoLink as Link } from "ardo/runtime";
import {
  MonoAction,
  MonoClosing,
  MonoColophon,
  MonoEngraving,
  MonoOpening,
} from "ferramenta-family/monografie";
import { WORKSHOP } from "ferramenta-family/registry";

import { ferrolink, lowerFirst, samples } from "../components/home-data";
import { Fit, Scanner } from "../components/home-engine";
import { Example, Explainer, Paths, Versions } from "../components/home-reading";
import { license, nodeVersions, rustVersion, version } from "../data/site.json";

// oxlint-disable-next-line react/only-export-components -- Ardo reads route metadata.
export const meta: ArdoMetaFunction = () => [
  { title: "Ferrolink — Catch broken links before they go live." },
  {
    name: "description",
    content:
      "A link checker for static sites. Ferrolink checks links, assets and anchors in your finished build against the files that are really there, from the command line, Node.js or Vite, without a server or network requests.",
  },
];
// oxlint-disable-next-line react/only-export-components -- Ardo reads the route layout.
export const handle = { layout: "bare" };

const releaseLine = version.split(".").slice(0, 2).join(".");
/** Every kind of issue the specimens show, from the published package's output. */
const issueKinds = new Set(
  samples.example.states.flatMap((state) => state.issues.map((issue) => issue.kind)),
);

function Actions() {
  return (
    <>
      <MonoAction
        href="/guide/getting-started/#add-a-vite-build-check"
        primary
        detail="npm install --save-dev ferrolink"
      >
        Check your Vite build
      </MonoAction>
      <MonoAction
        href="/guide/getting-started/#start-with-the-command-line"
        detail="cargo install --locked ferrolink"
      >
        Start with the command line
      </MonoAction>
    </>
  );
}

function Opening() {
  return (
    <MonoOpening
      product="ferrolink"
      instrumentLabel="Ferrolink's instrument: two steel posts on a shared base, joined by a chain of three links over a steel rail. The middle link is rust-brown and carries a small square gauge at the point where it holds the other two."
      kicker={`Ferrolink ${releaseLine}, ${lowerFirst(ferrolink.what)}`}
      title="Catch broken links before they go live."
      subtitle="Check the build your visitors will get."
      standfirst={`Documentation sites and blogs are built into a folder of HTML pages, images and scripts, and every page points at others. Rename a page or reword a heading, and some of those pointers lead nowhere. Ferrolink reads the finished folder and checks every link, asset and anchor against the files that are really there, without starting a server or making network requests. ${ferrolink.audience}`}
      actions={<Actions />}
      facts={[
        {
          value: (
            <>
              <em>{issueKinds.size}</em> kinds
            </>
          ),
          label: "of issue: a missing file, a missing anchor, a link outside the site’s path",
        },
        { value: "Offline", label: "links to other hosts are counted, never fetched" },
        { value: "CLI, Node.js, Vite", label: "one native checker behind all three" },
        {
          value: `Rust ${rustVersion}+`,
          label: `${license}; the npm package runs on Node.js ${nodeVersions.join(" or ")}`,
        },
      ]}
    />
  );
}

// Keys are written out: minified builds do not keep function names.
const chapters = [
  { key: "explainer", Chapter: Explainer },
  { key: "example", Chapter: Example },
  { key: "paths", Chapter: Paths },
  { key: "versions", Chapter: Versions },
  { key: "scanner", Chapter: Scanner },
  { key: "fit", Chapter: Fit },
];

/** Where Ferrolink comes from, and who makes it. */
function Origin() {
  return (
    <section aria-label="Origin">
      <div className="mono-axis">
        <MonoColophon>
          {ferrolink.proof} It is one of the Ferramenta engines, made by {WORKSHOP.name} in{" "}
          {WORKSHOP.place}.
        </MonoColophon>
      </div>
    </section>
  );
}

export default function HomePage() {
  return (
    <>
      <Opening />
      {chapters.map(({ key, Chapter }, index) => (
        <Fragment key={key}>
          <MonoEngraving seed={index + 1} />
          <Chapter />
        </Fragment>
      ))}
      <MonoEngraving seed={chapters.length + 1} />
      <Origin />
      <div id="start">
        <MonoClosing
          title="Check the build before it goes live."
          standfirst="Add the Vite plugin to stop a broken build, or run the command line after any static site generator. Both read the folder your build already wrote."
          actions={<Actions />}
          links={
            <>
              <a href="https://github.com/sebastian-software/ferrolink">GitHub repository</a>
              <Link to="/guide/getting-started/">Documentation</Link>
              <Link to="/reference/node/">Node.js and Vite reference</Link>
              <a href={WORKSHOP.consulting}>Integration and support</a>
            </>
          }
        />
      </div>
    </>
  );
}
