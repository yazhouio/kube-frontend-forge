import { Loading } from "@kubed/components";
import * as React from "react";
import { useNavigate } from "react-router-dom";

type IframeMessage = {
  type?: string;
  callback?: string;
};

export type IframeProps = {
  src: string;
};

export function BaseIframe({ src }: IframeProps) {
  const navigate = useNavigate();
  const iframeRef = React.useRef<HTMLIFrameElement | null>(null);
  const frameOrigin = new URL(src, window.location.href).origin;
  const isSameOrigin = frameOrigin === window.location.origin;
  const [loading, setLoading] = React.useState(isSameOrigin);

  React.useEffect(() => {
    setLoading(isSameOrigin);
  }, [isSameOrigin, src]);

  React.useEffect(() => {
    const listener = (event: MessageEvent) => {
      if (!isSameOrigin) {
        return;
      }

      if (event.origin !== frameOrigin) {
        return;
      }

      if (event.source !== iframeRef.current?.contentWindow) {
        return;
      }

      const message = event.data as IframeMessage | null;
      if (!message || typeof message !== "object") {
        return;
      }

      if (message.type === "console-iframe-login") {
        navigate("/login");
        return;
      }

      if (message.type !== "console-iframe-ready") {
        return;
      }

      if (typeof message.callback !== "string") {
        return;
      }

      iframeRef.current?.contentWindow?.postMessage(
        {
          type: message.callback,
          data: `
          .toolbox-root, .feedback-root, .root-layout-header {
                 display: none !important;
                 };
          `,
        },
        frameOrigin,
      );
    };

    window.addEventListener("message", listener);
    return () => {
      window.removeEventListener("message", listener);
    };
  }, [frameOrigin, isSameOrigin, navigate]);

  const onIframeLoad = () => {
    setLoading(false);
  };

  return (
    <>
      {loading && <Loading className="page-loading" />}
      <iframe
        ref={iframeRef}
        src={src}
        width="100%"
        height="100%"
        frameBorder="0"
        style={{
          height: "calc(100vh - 68px)",
          display: loading ? "none" : "block",
        }}
        onLoad={onIframeLoad}
      />
    </>
  );
}
