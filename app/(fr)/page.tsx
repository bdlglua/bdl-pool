import { Home } from "@/components/Home";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("home", "fr");

export default function Page() {
  return <Home lang="fr" />;
}
