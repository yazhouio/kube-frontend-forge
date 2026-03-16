import styled from "styled-components";

export const PageContainer = styled.div`
  padding: 20px;
  table .table-cell {
    word-break: break-word;
    .field-label {
      white-space: break-spaces;
      min-width: 200px;
    }
  }
  .kube-table-head.with-sticky {
    top: -20px;
    .table-cell {
      background-color: #fff;
      > div {
        line-height: 0px;
      }
    }
  }
  .kube-table-wrapper {
    overflow-x: auto;
  }
`;

export const PageLayout = styled.div`
  display: grid;
  grid-template-rows: auto 1fr;
  height: 100%;
  overflow: hidden;
  background-color: rgb(239, 244, 249);
`;

export const PageTitle = styled.div`
  color: rgb(36, 46, 66);
  padding: 14px 20px;
  font-size: 18px;
  line-height: 28px;
  font-weight: bold;
  background-color: #fff;
`;
