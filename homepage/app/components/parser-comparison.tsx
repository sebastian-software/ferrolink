import { type ReactNode, useState } from "ardo/react";
import {
  MonoBarChart,
  type MonoBarRow,
  MonoSegmented,
  MonoTableView,
} from "ferramenta-family/monografie";

import { samples } from "./home-data";

/* Time for the same work, relative to Ferrolink's Ferroni scanner, from the
   published tables in docs/parsers.md via app/data/home-samples.json. */

const { speed } = samples;
type TaskId = "check" | "scan";
const isTask = (value: string): value is TaskId => value === "check" || value === "scan";

/** The task in words, in the order of the toggles. */
const TASKS: Record<TaskId, { label: string; phrase: string; own: string }> = {
  scan: {
    label: "Read every page, one thread",
    phrase: "reading every page on one thread",
    own: "Ferrolink’s scanner, regular expressions",
  },
  check: {
    label: "The whole check, all cores",
    phrase: `the whole check on ${String(speed.measured.threads)} threads`,
    own: "Ferrolink’s scanner, regular expressions",
  },
};
const ORDER: TaskId[] = ["scan", "check"];
/** What each tokenizer is, from the table in docs/parsers.md. */
const SUBS: Record<string, string> = {
  html5gum: "a complete HTML tokenizer",
  lol_html: "Cloudflare’s streaming rewriter",
  html5ever: "the tokenizer of Servo’s browser engine",
};

function task(id: TaskId) {
  const found = speed.tasks.find((entry) => entry.id === id);
  if (!found) throw new Error(`The parser data has no ${id} task.`);
  return found;
}

const time = (value: number) => `${value.toLocaleString("en-US", { maximumFractionDigits: 1 })} ms`;

/** A round end for the scale, past the slowest bar, with ticks at its middle and end. */
function scaleFor(id: TaskId) {
  const slowest = Math.max(...task(id).rows.map((row) => row.ratio));
  const scale = [2, 3, 4, 5, 6, 8, 10].find((step) => step >= slowest) ?? 20;
  const ticks = [...new Set([1, scale / 2, scale])].filter((tick) => tick >= 1);
  return { scale, ticks };
}

function rowsFor(id: TaskId): MonoBarRow[] {
  return [
    { id: "ferroni", label: "Ferroni", sub: TASKS[id].own, value: 1, own: true },
    ...task(id).rows.map((row) => ({
      id: row.id,
      label: row.id,
      sub: SUBS[row.id] ?? "",
      value: row.ratio,
    })),
  ];
}

function describeFor(id: TaskId) {
  const { ferroni, rows } = task(id);
  return function describe(row: MonoBarRow, cut: boolean) {
    if (row.own === true) {
      return (
        <>
          <b>Ferroni</b>, the scanner Ferrolink ships, is the baseline: 1× for {TASKS[id].phrase},{" "}
          {time(ferroni)}.
        </>
      );
    }
    const measured = rows.find((entry) => entry.id === row.id);
    const relation = row.value < 1 ? "of the time Ferroni took" : "as long as Ferroni";
    return (
      <>
        <b>{row.label}</b> needed {row.value.toFixed(2)}× {relation} for {TASKS[id].phrase}
        {measured ? `, ${time(measured.value)}` : ""}
        {cut ? "; its bar runs past the end of the scale." : "."}
      </>
    );
  };
}

/** Every figure of the chart as a table, with the measured times. */
function Figures() {
  return (
    <MonoTableView>
      <table className="mono-table">
        <thead>
          <tr>
            <th scope="col">Task</th>
            <th scope="col">Scanner</th>
            <th scope="col" className="mono-num">
              Time
            </th>
            <th scope="col" className="mono-num">
              Relative
            </th>
          </tr>
        </thead>
        <tbody>
          {ORDER.flatMap((id) => {
            const { ferroni, rows } = task(id);
            return [
              <tr key={`${id}-ferroni`}>
                <td>{TASKS[id].label}</td>
                <td>Ferroni</td>
                <td className="mono-num">{time(ferroni)}</td>
                <td className="mono-num">1×</td>
              </tr>,
              ...rows.map((row) => (
                <tr key={`${id}-${row.id}`}>
                  <td>{TASKS[id].label}</td>
                  <td>{row.id}</td>
                  <td className="mono-num">{time(row.value)}</td>
                  <td className="mono-num">{row.ratio.toFixed(2)}×</td>
                </tr>
              )),
            ];
          })}
        </tbody>
      </table>
    </MonoTableView>
  );
}

export type ParserComparisonProps = {
  /** Text and links under the scale. */
  after: ReactNode;
  /** The measuring conditions beside the chart. */
  conditions: ReactNode;
};

export function ParserComparison({ after, conditions }: ParserComparisonProps) {
  const [id, setId] = useState<TaskId>("scan");
  const { scale, ticks } = scaleFor(id);
  return (
    <MonoBarChart
      label={`Time for ${TASKS[id].phrase}, relative to Ferroni`}
      title="Time for the same pages"
      description="Ferroni = 1×, the line through every row. A bar past it took longer, a shorter bar less time."
      toggles={
        <MonoSegmented
          label="Task"
          value={id}
          options={ORDER.map((value) => ({ value, label: TASKS[value].label }))}
          onChange={(value) => {
            if (isTask(value)) setId(value);
          }}
        />
      }
      rows={rowsFor(id)}
      scale={scale}
      ticks={ticks}
      describe={describeFor(id)}
      after={
        <>
          {after}
          <Figures />
        </>
      }
      conditions={conditions}
    />
  );
}
