import { useState } from "react";
import { RouterProvider, type RouterHistory } from "@tanstack/react-router";

import { createAppRouter } from "@/router";

export default function App({ history }: { history?: RouterHistory }) {
  const [router] = useState(() => createAppRouter(history));
  return <RouterProvider router={router} />;
}
