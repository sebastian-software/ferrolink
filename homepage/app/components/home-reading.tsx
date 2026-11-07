import { ArdoLink as Link } from "ardo/runtime";
import {
  MonoCredit,
  MonoLeadIn,
  MonoPassage,
  MonoSection,
  MonoTerms,
  MonoWide,
} from "ferramenta-family/monografie";

import { ExampleSpecimen, PathsSpecimen, VersionsSpecimen } from "./check-specimens";
import { samples, spelled } from "./home-data";

/* The homepage's reading chapters: what a broken reference is, a failed
   check, host paths, and old versions with their deployment files. */

const { paths, versions } = samples;
/** The one issue left once both rules for old versions are in place. */
const kept = versions.states.at(-1)?.issues[0];

const WHAT_IT_READS = [
  {
    term: "Links and assets",
    description:
      "Every address a page points at: links, images and responsive images, scripts, stylesheets, video posters, embedded frames and the preview image a social network shows.",
  },
  {
    term: "Anchors",
    description:
      "The part after the #. It must name an element on the target page, so a link lands on the section it promises.",
  },
  {
    term: "The site’s path",
    description:
      "On a site served under a folder such as /docs/, a link that forgets the folder works on your laptop and fails online.",
  },
  {
    term: "Deployment files",
    description:
      "Ardo’s versions.json and the redirect rules in _redirects and vercel.json, whose destinations are links too.",
  },
];

export function Explainer() {
  return (
    <MonoSection
      id="overview"
      title="A finished site is a folder of pages that point at each other."
    >
      <MonoPassage
        note={{
          term: "Static site",
          children:
            "A website whose pages are finished files on disk. The host sends them as they are; nothing is computed when a visitor asks for a page.",
        }}
      >
        <p>
          <MonoLeadIn>Documentation and blogs</MonoLeadIn> are often built by a static site
          generator: a program that turns Markdown into a folder of HTML pages, images and scripts.
          A host such as GitHub Pages or Netlify then serves that folder as it is. Every page in it
          points at others: a link to the next chapter, an image, a stylesheet, a jump to a heading
          further down.
        </p>
      </MonoPassage>
      <MonoPassage
        note={{
          side: "right",
          term: "Anchor",
          children:
            "The end of a link after the #, as in /guide/setup#install. The browser scrolls to the element with that id. When there is none, it quietly stays at the top of the page.",
        }}
      >
        <p>
          Rename a page, reword a heading or move the site under a new path, and some of those
          pointers lead nowhere. A visitor gets an error page, an empty frame where an image should
          be, or the top of a long page instead of the section they wanted. The Markdown sources
          cannot tell you all of this, because the generator decides the final file names and
          addresses. Ferrolink reads the finished folder and checks each reference against the files
          that are really there.
        </p>
      </MonoPassage>
      <MonoWide>
        <MonoTerms prose columns={2} items={WHAT_IT_READS} />
      </MonoWide>
      <MonoCredit>
        <Link to="/reference/cli/#what-it-checks">Every element and attribute it reads</Link>
      </MonoCredit>
    </MonoSection>
  );
}

export function Example() {
  return (
    <MonoSection
      id="example"
      title="Three small mistakes. One failed check."
      standfirst="A missing anchor, a missing image and a link that forgets the site’s path. Switch between the same page before and after the fix."
    >
      <MonoPassage
        note={{
          term: "Base path",
          children:
            "The folder a site is served under, like /docs/ in example.org/docs/. GitHub Pages serves project sites this way.",
        }}
      >
        <p>
          <MonoLeadIn>Each issue names</MonoLeadIn> the file and line it was found on, what is wrong
          and the reference as it was written. In a pipeline, that is enough to stop the deployment:
          the command line exits with 1 when it finds issues, and the Vite plugin fails the build.
          The page below sits on a site served under <code>/docs/</code>, so a link to{" "}
          <code>/guide/setup</code> leaves it, however right it looks.
        </p>
      </MonoPassage>
      <ExampleSpecimen />
      <MonoCredit>
        <Link to="/guide/getting-started/">Run the check on your own build</Link>
      </MonoCredit>
    </MonoSection>
  );
}

export function Paths() {
  return (
    <MonoSection
      id="paths"
      title="Checked the way your host will serve it."
      standfirst="A link to /guide can mean guide.html, a folder’s index.html or nothing at all. Ferrolink follows the rules that static hosts share."
    >
      <MonoPassage
        note={{
          term: "Extensionless address",
          children:
            "A link without .html at the end, such as /docs/guide. Most static hosts look for a matching file and fill in the rest.",
        }}
      >
        <p>
          <MonoLeadIn>GitHub Pages, Netlify</MonoLeadIn>, Cloudflare Pages and most static servers
          agree on how an address becomes a file. An address that ends in a slash serves the
          folder’s <code>index.html</code>. Any other address serves the exact file if there is one,
          then the same name with <code>.html</code>, then a folder’s <code>index.html</code>.
          Ferrolink resolves every reference by the same rules, so it finds what a visitor would
          find.
        </p>
      </MonoPassage>
      <MonoPassage
        note={{
          side: "right",
          term: "Case-sensitive",
          children:
            "Guide and guide are two different names. Many laptop disks treat them as one; the hosts above do not.",
        }}
      >
        <p>
          Names are compared exactly, as on those hosts, even when your own disk would let a wrong
          capital letter pass. Give Ferrolink the site’s public address, and links that spell out
          your own host, such as a canonical link, are checked like any other path. Here is one page
          with {spelled(paths.links.length)} links, checked against a build of{" "}
          {spelled(paths.files.length)} files.
        </p>
      </MonoPassage>
      <PathsSpecimen />
      <MonoCredit>
        <Link to="/reference/cli/#how-urls-map-to-files">How addresses map to files</Link>
      </MonoCredit>
    </MonoSection>
  );
}

/** The files that steer a host, and the one issue both rules leave. */
function HostFiles() {
  return (
    <MonoPassage
      note={{
        side: "right",
        term: "versions.json",
        children:
          "The list Ardo’s version switcher reads. Each entry is a path, and Ferrolink checks that it leads somewhere.",
      }}
    >
      <p>
        The files that steer a host count as well. A redirect to a page that no longer exists sends
        visitors to an error just like a broken link does. Switch between the three checks below:
        with both rules in place, one issue is left
        {kept ? (
          <>
            , the redirect in <code>{kept.source}</code> to <code>{kept.href}</code>, a page the
            build does not have.
          </>
        ) : (
          "."
        )}
      </p>
    </MonoPassage>
  );
}

export function Versions() {
  return (
    <MonoSection
      id="versions"
      title="Old versions stay online. Their links need rules."
      standfirst="Documentation keeps earlier versions next to the current one. Ferrolink can leave an archive alone, accept a version that lives elsewhere, and still check what your host is told to serve."
    >
      <MonoPassage
        note={{
          term: "Redirect file",
          children:
            "A file such as _redirects on Netlify and Cloudflare Pages, or vercel.json, that tells the host to send one address to another.",
        }}
      >
        <p>
          <MonoLeadIn>An archived version</MonoLeadIn> was correct when it shipped, and nobody will
          edit it again. Its links may point at pages that have moved on since. A check that fails
          on every old page teaches a team to stop reading the check, so Ferrolink offers two
          separate choices. <code>exclude</code> skips the links inside matching pages, while links
          into them still have their anchors checked. <code>assumePresent</code> accepts links to
          paths that are deployed separately, without looking for them in the build.
        </p>
      </MonoPassage>
      <HostFiles />
      <VersionsSpecimen />
      <MonoCredit>
        <Link to="/reference/cli/#usage">Every option for archived and separate versions</Link>
      </MonoCredit>
    </MonoSection>
  );
}
