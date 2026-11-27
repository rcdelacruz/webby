import { getCollection, type CollectionEntry } from "astro:content";

export type Release = CollectionEntry<"releases">;

/** Every release, newest first. */
export async function allReleases(): Promise<Release[]> {
  const releases = await getCollection("releases");
  return releases.sort((a, b) => b.data.date.getTime() - a.data.date.getTime());
}

/** The id a release's section has on the changelog page. */
export const anchor = (release: Release) => `v${release.data.version.replaceAll(".", "-")}`;

export const longDate = (date: Date) => date.toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });
