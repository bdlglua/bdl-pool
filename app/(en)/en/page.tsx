import { Home } from "@/components/Home";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("home", "en");

export default function Page() {
  return <Home lang="en" />;
}
