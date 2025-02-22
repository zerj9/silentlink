// /app/auth-callback/route.ts
import { NextResponse } from "next/server";

export async function GET(request: Request) {
  // Parse URL and extract search parameters
  const { searchParams } = new URL(request.url);
  const code = searchParams.get("code");
  const state = searchParams.get("state");
  const scope = searchParams.get("scope");
  const authuser = searchParams.get("authuser");
  const hd = searchParams.get("hd");
  const prompt = searchParams.get("prompt");

  console.log("Authentication Callback Parameters:");
  console.log("code:", code);
  console.log("state:", state);
  console.log("scope:", scope);
  console.log("authuser:", authuser);
  console.log("hd:", hd);
  console.log("prompt:", prompt);

  // Validate required parameters
  if (!code || !state) {
    console.error("Missing code or state parameter in callback URL.");
    return new NextResponse("Error: Missing code or state parameter.", { status: 400 });
  }

  let session_id: string;

  // Perform server-side authentication callback logic
  try {
    const response = await fetch("http://localhost:3210/oidc/callback", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ code, state, scope, authuser, hd, prompt }),
    });

    if (!response.ok) {
      throw new Error(`Callback failed with status ${response.status}`);
    }

    // Assume the backend returns the session id in the response body
    session_id = await response.json();
    console.log("Authentication callback succeeded");
  } catch (error) {
    console.error("Error during authentication callback:", error);
    return new NextResponse("Error processing authentication callback.", { status: 500 });
  }

  // Create a redirect response and set the "sid" cookie.
  const response = NextResponse.redirect(new URL("/", request.url));

  response.cookies.set("sid", session_id, {
    path: "/",
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "lax",
  });

  return response;
}
