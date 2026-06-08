import { useMemo } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";

export function usePageRuntimeRouter() {
  const location = useLocation();
  const navigate = useNavigate();
  const params = useParams();

  return useMemo(() => {
    return {
      route: {
        current: location.pathname,
        params,
      },
      navigation: {
        navigate: (to: string | number) => {
          navigate(to as any);
        },
        goBack: () => {
          navigate(-1);
        },
      },
    };
  }, [location]);
}
