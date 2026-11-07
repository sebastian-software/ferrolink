import { useState } from "ardo/react";
import {
  MonoCaption,
  MonoFacing,
  MonoSegmented,
  MonoSheet,
  MonoWide,
} from "ferramenta-family/monografie";

import { capitalized, kindLabel, renderedWith, samples, spelled } from "./home-data";
import "./check-specimens.css";

/* The homepage's specimens: every page, option, issue and count here is real
   output of the published ferrolink package, from app/data/home-samples.json. */

const { example, paths, versions } = samples;

/** Numbered lines of code or of a file; a long line wraps under a hanging indent. */
function Lines({ text }: { text: string }) {
  const lines = text.endsWith("\n") ? text.slice(0, -1).split("\n") : text.split("\n");
  return (
    <ol className="fl-lines">
      {lines.map((line, index) => {
        // Indentation becomes padding, so a wrapped line never breaks after its spaces.
        const indent = line.length - line.trimStart().length;
        return (
          <li key={`${String(index)}:${line}`}>
            <code style={{ paddingLeft: `${String(indent + 4)}ch` }}>{line.trimStart()}</code>
          </li>
        );
      })}
    </ol>
  );
}

type Issue = (typeof example.states)[number]["issues"][number];

/** A check's issues, grouped by the file they were found in, as the command line groups them. */
function Issues({ issues }: { issues: Issue[] }) {
  if (issues.length === 0) return <p className="fl-clear">None. Every reference resolves.</p>;
  const sources = [...new Set(issues.map(({ source }) => source))];
  return (
    <div className="fl-issues">
      {sources.map((source) => (
        <div key={source}>
          <p className="fl-source">
            <code>{source}</code>
          </p>
          <ul>
            {issues
              .filter((issue) => issue.source === source)
              .map((issue) => (
                <li key={`${String(issue.line)}:${issue.href}`}>
                  <span className="fl-where">
                    line {issue.line} · <b>{kindLabel(issue.kind)}</b>
                  </span>
                  <code>{issue.href}</code>
                </li>
              ))}
          </ul>
        </div>
      ))}
    </div>
  );
}

const count = (value: number, one: string, many: string) =>
  `${spelled(value)} ${value === 1 ? one : many}`;
const issueLabel = (issues: number) =>
  issues === 0 ? "No issues found" : capitalized(`${count(issues, "issue", "issues")} found`);

const EXAMPLE_LABELS: Record<string, string> = { before: "Before the fix", after: "After the fix" };

type ExampleState = (typeof example.states)[number];

/** What one check of the example found, and what it counted. */
function ExampleResult({ state }: { state: ExampleState }) {
  const { summary } = state;
  return (
    <MonoSheet label={issueLabel(summary.issues)} meta="report.issues">
      <Issues issues={state.issues} />
      <p className="fl-sub">Counted, from report.summary</p>
      <ul className="fl-counts">
        <li>{count(summary.pages, "page", "pages")} checked</li>
        <li>{count(summary.internalLinks, "reference", "references")} inside the site</li>
        <li>{count(summary.externalLinks, "external URL", "external URLs")}, not fetched</li>
      </ul>
    </MonoSheet>
  );
}

/** The rest of the example build, folded under the result. */
function ExampleFiles() {
  return (
    <details className="fl-details">
      <summary>Show the other files of the build</summary>
      <MonoFacing className="fl-facing">
        {example.files.map(({ path, text }) => (
          <MonoSheet key={path} label="In the build" meta={path}>
            <Lines text={text} />
          </MonoSheet>
        ))}
      </MonoFacing>
    </details>
  );
}

/** One page with three small mistakes, before and after fixing them. */
export function ExampleSpecimen() {
  const [id, setId] = useState("before");
  const current = example.states.find((entry) => entry.id === id) ?? example.states[0];
  if (!current) throw new Error("The example sample has no builds.");
  return (
    <MonoWide>
      <div className="fl-toggles">
        <MonoSegmented
          label="Build"
          value={current.id}
          options={example.states.map((entry) => ({
            value: entry.id,
            label: EXAMPLE_LABELS[entry.id] ?? entry.id,
          }))}
          onChange={setId}
        />
      </div>
      <MonoFacing className="fl-facing" aria-live="polite">
        <MonoSheet label="The page, as built" meta="index.html">
          <Lines text={current.page} />
        </MonoSheet>
        <ExampleResult state={current} />
      </MonoFacing>
      <MonoCaption>
        Real output of the ferrolink npm package {renderedWith}, checked with{" "}
        <code>siteUrl: "{example.siteUrl}"</code>. The build also holds{" "}
        {example.files.map(({ path }, index) => (
          <span key={path}>
            {index > 0 && " and "}
            <code>{path}</code>
          </span>
        ))}
        .
      </MonoCaption>
      <ExampleFiles />
    </MonoWide>
  );
}

/** Links as written, and what the check said about each. */
export function PathsSpecimen() {
  const found = paths.links.filter(({ kind }) => kind === null).length;
  return (
    <MonoWide>
      <MonoFacing className="fl-single">
        <MonoSheet
          label={capitalized(
            `${count(paths.links.length, "link", "links")}, ${spelled(found)} found`,
          )}
          meta={`siteUrl: ${paths.siteUrl}`}
        >
          <ul className="fl-verdicts">
            {paths.links.map(({ href, kind }) => (
              <li key={href} data-found={kind === null ? "" : undefined}>
                <code>{href}</code>
                <span>{kind === null ? "found" : kindLabel(kind)}</span>
              </li>
            ))}
          </ul>
        </MonoSheet>
      </MonoFacing>
      <MonoCaption>
        Real output of the ferrolink npm package {renderedWith}. The build holds{" "}
        {paths.files.map((path, index) => (
          <span key={path}>
            {index === 0 ? "" : index === paths.files.length - 1 ? " and " : ", "}
            <code>{path}</code>
          </span>
        ))}
        , and every link stands on <code>index.html</code>.
      </MonoCaption>
    </MonoWide>
  );
}

const VERSION_LABELS: Record<string, string> = {
  every: "Check every page",
  exclude: "Skip the archive",
  assume: "Accept the separate deployment",
};

type VersionOptions = { exclude?: string[]; assumePresent?: string[] };

function callSource(options: VersionOptions) {
  const lines = ["const report = await check({", '  root: "site",'];
  for (const [name, globs] of Object.entries(options)) {
    lines.push(`  ${name}: [${globs.map((glob) => JSON.stringify(glob)).join(", ")}],`);
  }
  lines.push("});");
  return lines.join("\n");
}

/** A versioned site with an archive, checked three ways. */
export function VersionsSpecimen() {
  const [id, setId] = useState("every");
  const current = versions.states.find((entry) => entry.id === id) ?? versions.states[0];
  if (!current) throw new Error("The versions sample has no checks.");
  return (
    <MonoWide>
      <div className="fl-toggles">
        <MonoSegmented
          label="Check"
          value={current.id}
          options={versions.states.map((entry) => ({
            value: entry.id,
            label: VERSION_LABELS[entry.id] ?? entry.id,
          }))}
          onChange={setId}
        />
      </div>
      <MonoFacing className="fl-facing" aria-live="polite">
        <MonoSheet label="The check, in Node.js" meta="check()">
          <Lines text={callSource(current.options)} />
        </MonoSheet>
        <MonoSheet label={issueLabel(current.summary.issues)} meta="report.issues">
          <Issues issues={current.issues} />
        </MonoSheet>
      </MonoFacing>
      <MonoCaption>
        Real output of the ferrolink npm package {renderedWith}, on a copy of the build below in a
        temporary folder. Version 2 is current, version 1 is an archive with an old link, and
        version 0 is listed in <code>versions.json</code> but deployed somewhere else.
      </MonoCaption>
      <details className="fl-details">
        <summary>Show the build’s files, versions.json and _redirects</summary>
        <MonoFacing className="fl-facing">
          <MonoSheet label="The build" meta={`${String(versions.files.length)} files`}>
            <Lines text={versions.files.join("\n")} />
          </MonoSheet>
          <MonoSheet label="The version list" meta="versions.json">
            <Lines text={versions.manifest} />
          </MonoSheet>
          <MonoSheet label="The host’s redirects" meta="_redirects">
            <Lines text={versions.redirects} />
          </MonoSheet>
        </MonoFacing>
      </details>
    </MonoWide>
  );
}
