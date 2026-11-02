import type { ArdoMetaFunction } from "ardo/runtime";

import { ArdoLink as Link } from "ardo/runtime";
import { Mark, RegistryFacts } from "ferramenta-family";
import {
  FerraArticle,
  FerraChoices,
  FerraCode,
  FerraColumns,
  FerraEditorialRow,
  FerraFeatureGrid,
  FerraInstrument,
  FerraNotes,
  FerraProductLead,
  FerraProductPage,
  FerraReleaseFacts,
} from "ferramenta-family/ferra";
import { family, WORKSHOP } from "ferramenta-family/registry";

import { CheckExample } from "../components/check-example";
import { SiteAction } from "../components/site-action";
import site from "../data/site.json";

// oxlint-disable-next-line react/only-export-components -- Ardo reads route metadata.
export const meta: ArdoMetaFunction = () => [
  { title: "Ferrolink — Catch broken links before they go live." },
  {
    name: "description",
    content:
      "Check links, assets and anchors in your finished static build. A native CLI, Node.js API and Vite plugin, without a server or network requests.",
  },
];
// oxlint-disable-next-line react/only-export-components -- Ardo reads the route layout.
export const handle = { layout: "bare" };

const member = family.find((entry) => entry.name === "ferrolink");
if (!member) throw new Error("Ferrolink is missing from the family registry.");
const tool = { ...member, version: site.version };

function Opening() {
  return (
    <FerraProductLead
      id="overview"
      title="Catch broken links before they go live."
      introduction="A link checker for finished static sites. Ferrolink checks page links, assets and anchors against the files your build actually produced. Run it from the command line, Node.js or Vite, without starting a server or making network requests."
      art={<FerraInstrument product="ferrolink" />}
      byline={
        <>
          An open-source project by <a href={WORKSHOP.openSource}>{WORKSHOP.company}</a>.
        </>
      }
    >
      <div className="ferra-actions">
        <SiteAction to="/guide/getting-started/">Check your first build</SiteAction>
        <SiteAction variant="secondary" to="#example">
          See what it catches
        </SiteAction>
      </div>
    </FerraProductLead>
  );
}

function Highlights() {
  return (
    <FerraColumns>
      <FerraArticle
        title="The page exists. Does the anchor?"
        icon={<Mark name="document" size={28} />}
        meta={<FerraReleaseFacts tool={tool} />}
      >
        <p>
          A link can reach a real page and still leave the reader in the wrong place. Ferrolink
          checks the target element too, alongside image sources, responsive images, scripts and
          other local assets.
        </p>
        <Link to="/reference/cli/#what-it-checks">See exactly what is checked</Link>
      </FerraArticle>
      <FerraArticle
        title="Check the paths your host will serve."
        icon={<Mark name="package" size={28} />}
        meta={<Link to="/reference/cli/#how-urls-map-to-files">Static-host routing rules</Link>}
      >
        <p>
          Project paths, directory indexes and extensionless URLs matter after deployment. Set your
          public URL and Ferrolink can catch links that forget the base path, plus missing
          destinations in supported deployment files.
        </p>
        <Link to="/guide/getting-started/">Add the check after your build</Link>
      </FerraArticle>
    </FerraColumns>
  );
}

function Scope() {
  return (
    <FerraEditorialRow
      notes={<FerraNotes label="Inside Ferrolink" notes={tool.editorial?.notes ?? []} />}
    >
      <FerraArticle
        title="One finished directory. A check you can repeat."
        introduction="Source files cannot tell you everything about the site your generator will publish. Check the artifact itself, locally or in CI."
        meta={<Link to="/reference/cli/#usage">Options for your deployment</Link>}
      >
        <FerraFeatureGrid
          items={[
            {
              heading: "No server to bring up",
              text: "Read the build on disk. Internal references are resolved locally; external URLs are counted, without being fetched.",
            },
            {
              heading: "Old versions, explicit rules",
              text: "Skip checking links inside archived pages, or accept targets that are deployed separately. Those are distinct choices.",
            },
            {
              heading: "Deployment files count too",
              text: "Check version paths in Ardo’s versions.json and local redirect or rewrite destinations in _redirects and vercel.json.",
            },
            {
              heading: "A useful result for CI",
              text: "Get source files, line numbers and issue kinds. Use a failing exit code, JSON output or the JavaScript report to act on them.",
            },
          ]}
        />
      </FerraArticle>
    </FerraEditorialRow>
  );
}

function Scanner() {
  return (
    <FerraArticle
      title="Built for generated HTML. Measured on real pages."
      meta={<Link to="/evidence/parsers/">Parser comparison, measurements and known limits</Link>}
    >
      <p>
        Ferrolink walks the build with Ferralk and scans pages in parallel with Ferroni. The scanner
        collects the references and anchors it needs, without constructing a DOM. A documented
        comparison with three HTML tokenizers explains that choice.
      </p>
      <p>
        This is a build check, not a full HTML validator or a live-site crawler. The parser decision
        records how malformed markup and named character references can differ from a browser.
        External availability and JavaScript-generated links remain outside the check.
      </p>
    </FerraArticle>
  );
}

function GettingStarted() {
  return (
    <FerraChoices>
      <FerraArticle
        title="Make it part of your Vite build."
        introduction="Install the npm package with its prebuilt native binary. The plugin checks the finished client output and fails the build when it finds issues."
      >
        <FerraCode caption="Install for Node.js">npm install --save-dev ferrolink</FerraCode>
        <SiteAction to="/guide/getting-started/#add-a-vite-build-check">
          Add the Vite plugin
        </SiteAction>
        <p>
          <Link to="/reference/node/#javascript-api">Need the JavaScript API instead?</Link>
        </p>
      </FerraArticle>
      <FerraArticle
        title="Use it with any static build."
        introduction="Install the Rust command-line tool and point it at your output directory. Run it yourself or add it as the next step in CI."
      >
        <FerraCode caption="Install and check">
          {"cargo install --locked ferrolink\nferrolink check dist"}
        </FerraCode>
        <SiteAction variant="secondary" to="/guide/getting-started/#start-with-the-command-line">
          Start with the CLI
        </SiteAction>
      </FerraArticle>
    </FerraChoices>
  );
}

export default function HomePage() {
  return (
    <RegistryFacts>
      <FerraProductPage>
        <Opening />
        <Highlights />
        <CheckExample />
        <Scope />
        <Scanner />
        <GettingStarted />
      </FerraProductPage>
    </RegistryFacts>
  );
}
