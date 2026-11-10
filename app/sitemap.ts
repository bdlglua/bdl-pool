import type { MetadataRoute } from "next";
import { origin, pageNames, route } from "@/lib/pages";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  return (["fr", "en"] as const).flatMap((lang) =>
    pageNames.map((page) => ({
      url: origin + route(page, lang),
      alternates: {
        languages: {
          fr: origin + route(page, "fr"),
          en: origin + route(page, "en"),
        },
      },
    })),
  );
}
