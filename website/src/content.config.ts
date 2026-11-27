import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";

// One file per release, named after its version. The newest one is what
// the home page presents as the current release.
const releases = defineCollection({
  loader: glob({ pattern: "*.md", base: "./src/content/releases" }),
  schema: z.object({
    version: z.string(),
    channel: z.enum(["alpha", "beta", "stable"]),
    date: z.coerce.date(),
    summary: z.string(),
    /** Heading of the home page section about this release. */
    headline: z.string().optional(),
    highlights: z.array(z.object({ title: z.string(), text: z.string(), visual: z.enum(["media", "fps", "scroll", "scale"]).optional() })).default([]),
  }),
});

const docs = defineCollection({
  loader: glob({ pattern: "*.md", base: "./src/content/docs" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
  }),
});

export const collections = { releases, docs };
