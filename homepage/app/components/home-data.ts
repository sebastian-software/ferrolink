import { family } from "ferramenta-family/registry";

import homeSamples from "../data/home-samples.json";

/* Everything the homepage shows is generated, never edited by hand: the check
   specimens and the parser comparison in app/data/home-samples.json
   (scripts/render-home-samples.mjs), the release facts in app/data/site.json
   (scripts/write-site-data.mjs, at build time). */
export const samples = homeSamples;

/** The npm package version that rendered the specimens. */
export const renderedWith = samples.renderedWith.ferrolink;

const member = family.find((entry) => entry.name === "ferrolink");
if (!member) throw new Error("Ferrolink is missing from the family registry.");
export const ferrolink = member;

const application = family.find((entry) => entry.name === "ardo");
if (!application?.runsOn?.includes("ferrolink")) {
  throw new Error("The registry no longer says that Ardo runs on Ferrolink.");
}
export const ardo = application;

/** A family member the registry says Ferrolink uses. */
export function engine(name: string) {
  const found = family.find((entry) => entry.name === name);
  if (!found || !ferrolink.uses?.includes(name)) {
    throw new Error(`The registry no longer says that Ferrolink uses ${name}.`);
  }
  return found;
}

/** A registry note by its title, for the margin. */
export function registryNote(title: string) {
  const note = ferrolink.editorial?.notes.find((entry) => entry.title === title);
  if (!note) throw new Error(`The registry has no Ferrolink note "${title}".`);
  return note;
}

/** The issue kinds as the command line and formatIssues() print them. */
export const kindLabel = (kind: string) => kind.replaceAll("_", " ");

const NUMBERS = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
/** Small counts in words, as the reading text writes them. */
export const spelled = (count: number) => NUMBERS[count] ?? count.toLocaleString("en-US");

/** The first letter in upper case, for a label that starts with a count. */
export const capitalized = (text: string) => `${text.charAt(0).toUpperCase()}${text.slice(1)}`;

/** The first letter in lower case, to set a registry phrase inside a sentence. */
export const lowerFirst = (text: string) => `${text.charAt(0).toLowerCase()}${text.slice(1)}`;
