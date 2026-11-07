import { ArdoLink as Link } from "ardo/runtime";
import {
  MonoCredit,
  MonoLeadIn,
  MonoPassage,
  MonoSection,
  MonoTerms,
  MonoWide,
} from "ferramenta-family/monografie";

import { license, nodeVersions, version } from "../data/site.json";
import { ardo, engine, ferrolink, lowerFirst, registryNote, samples } from "./home-data";
import { ParserComparison } from "./parser-comparison";

/* The homepage's engine chapters: how pages are read, and where Ferrolink fits. */

const { measured, tasks } = samples.speed;
const measuredOn = new Date(`${measured.date}T00:00:00Z`).toLocaleDateString("en-US", {
  year: "numeric",
  month: "long",
  day: "numeric",
  timeZone: "UTC",
});
const ferralk = engine("ferralk");
const ferroni = engine("ferroni");

/** The tokenizers that finished the whole check ahead of Ferroni, if any. */
const aheadOnAllCores = (tasks.find((task) => task.id === "check")?.rows ?? [])
  .filter((row) => row.ratio < 1)
  .map((row) => row.id);

/** How the parser figures were measured, from docs/parsers.md. */
function Conditions() {
  return (
    <>
      <h3>How this was measured</h3>
      <p>
        Recorded on {measuredOn} on an {measured.machine}, with Ferrolink {measured.ferrolink}{" "}
        during its development, before that version was released, and Ferroni {measured.ferroni}.
        Commit <code>{measured.revision}</code> is the last that holds all four scanners.
      </p>
      <p>
        The pages were the {measured.pages.toLocaleString("en-US")} pages of a build of the Ardo
        documentation, {measured.megabytes.toLocaleString("en-US")} MB of HTML. Every scanner fed
        the same code that collects links and anchors. The whole check ran on {measured.threads}{" "}
        threads and includes finding the files and checking every reference.
      </p>
      <p>
        The current release, {version}, has not been measured against the tokenizers: they were
        removed from the code once the decision was made.
      </p>
    </>
  );
}

/** What a link checker needs from a page, and how the scanner was chosen. */
function ScannerReading() {
  const note = registryNote("Small engines, one useful check");
  return (
    <>
      <MonoPassage note={{ term: note.title, children: note.body }}>
        <p>
          <MonoLeadIn>A browser</MonoLeadIn> turns HTML into a tree of elements before it shows
          anything, and a full HTML parser does the same work. A link checker needs a small part of
          it: the attributes that hold addresses, every <code>id</code>, and the first{" "}
          <code>{"<base>"}</code>. Ferrolink finds the files of the build with Ferralk,{" "}
          {lowerFirst(ferralk.what)}, and reads the pages in parallel with Ferroni,{" "}
          {lowerFirst(ferroni.what)}. One expression finds comments and start tags, a second splits
          the attributes of the tags that matter.
        </p>
      </MonoPassage>
      <MonoPassage
        note={{
          side: "right",
          term: "Tokenizer",
          children:
            "The first stage of an HTML parser. It cuts the text into tags, attributes and text, following the rules a browser uses.",
        }}
      >
        <p>
          That choice was measured against three HTML tokenizers on the same pages. On one thread,
          the regular expressions read them fastest. On all cores the whole check also spends its
          time finding files and checking references, and the gap closes
          {aheadOnAllCores.length > 0 ? (
            <>
              : {aheadOnAllCores.join(" and ")} finished slightly ahead. Ferroni stayed, because it
              is the engine the family builds, so every check Ferrolink runs also tests it.
            </>
          ) : (
            "."
          )}
        </p>
      </MonoPassage>
    </>
  );
}

export function Scanner() {
  return (
    <MonoSection
      id="scanner"
      title="Two regular expressions read every page."
      standfirst="Ferrolink needs three things from a page: the addresses it points at, the anchors it offers and its base address. It never builds the page the way a browser does."
    >
      <ScannerReading />
      <MonoWide>
        <ParserComparison
          after={
            <p>
              The full comparison also covers broken markup, how the patterns were tuned and three
              fixes to Ferroni that came out of it.{" "}
              <Link to="/evidence/parsers/">Every table, its method and its limits</Link>
            </p>
          }
          conditions={<Conditions />}
        />
      </MonoWide>
      <p>
        The price is known and small for generated pages. On markup with an unclosed quote, the
        expressions pick up at the next tag where a browser would not, and they decode only the
        character references that occur in addresses. On both builds of the Ardo documentation that
        were measured, every scanner reported the same issues.
      </p>
    </MonoSection>
  );
}

/** Where Ferrolink fits, and what it leaves out. */
const FIT_TERMS = [
  {
    term: "On the command line",
    description: (
      <>
        <code>cargo install --locked ferrolink</code>, then <code>ferrolink check dist</code> after
        any static site generator. JSON output is there for CI annotations.
      </>
    ),
  },
  {
    term: "In Node.js",
    description: (
      <>
        <code>npm install --save-dev ferrolink</code> brings a prebuilt native binary for macOS,
        Linux and Windows, without a Rust toolchain. It needs Node.js {nodeVersions.join(" or ")}.
      </>
    ),
  },
  {
    term: "In a Vite build",
    description:
      "The plugin checks the finished client output once the build is complete and fails the build when it finds issues, or only warns if you ask.",
  },
  {
    term: "In Ardo",
    description: (
      <>
        <a href={ardo.docs}>Ardo</a>, {lowerFirst(ardo.what)}, runs its link check on Ferrolink.
      </>
    ),
  },
  {
    term: "Not included",
    description:
      "Fetching external addresses to see whether they are online, running a page’s JavaScript, or validating HTML against the full standard.",
  },
  {
    term: "License",
    description: `${license}, for the command line, the library and the npm package.`,
  },
];

export function Fit() {
  const note = registryNote("A release check became a tool");
  return (
    <MonoSection id="fit" title="A build check, not a crawler.">
      <p>
        Ferrolink is {lowerFirst(ferrolink.what)}. It {lowerFirst(ferrolink.does)} It reads what
        your build wrote and leaves the internet alone: links to other hosts are counted, never
        fetched.
      </p>
      <MonoWide>
        <MonoTerms prose columns={2} items={FIT_TERMS} />
      </MonoWide>
      <MonoPassage note={{ term: note.title, children: note.body }}>
        <p>
          <MonoLeadIn>Ferrolink is young.</MonoLeadIn> Version {version} is an early release. Before
          1.0, a breaking change comes with a new minor version and is called out in the changelog.
          The JavaScript API and the Vite plugin are the supported contract; the Rust library serves
          the command line and the npm package and can change in any minor release. This website is
          checked by Ferrolink on every build.
        </p>
      </MonoPassage>
      <MonoCredit>
        <Link to="/reference/node/#api-stability">The stability policy</Link>
      </MonoCredit>
    </MonoSection>
  );
}
