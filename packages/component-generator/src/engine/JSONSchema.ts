interface BaseSchema {
  $id?: string;
  $schema?: string;
}

interface DataSchema extends BaseSchema {
  type: "object" | "array" | "string" | "number" | "boolean";
  properties?: Record<string, DataSchema>;
  items?: DataSchema;
  required?: string[];
  description?: string;
  $path?: string[];
}

export type NodeDefinitionSchema = {
  templateInputs?: Record<string, DataSchema>;
  runtimeProps?: Record<string, DataSchema>;
};

export type DataSourceDefinitionSchema = NodeDefinitionSchema & {
  outputs?: Record<string, DataSchema>;
};

// 对应上面 JSON Schema 的 TypeScript 类型
export interface PageConfig {
  meta: PageMeta;
  dataSources?: DataSourceNode[];
  actionGraphs?: ActionGraphSchema[];
  root: ComponentNode;
  context: Record<string, any>;
}

interface PageMeta {
  id: string;
  name: string;
  title?: string;
  description?: string;
  path?: string;
  // layout?: "default" | "blank" | "sidebar" | "dashboard";
  // permissions?: string[];
}

export interface DataSourceNode {
  id: string;
  type: string;
  config: Record<string, any>; // todo: 根据 type 不同，config 的类型不同
  args?: PropValue[];
  autoLoad?: boolean;
  polling?: {
    enabled: boolean;
    interval?: number;
  };
}

export interface ComponentNode {
  id: string;
  type: string;
  props?: Record<string, PropValue>;
  meta?: {
    scope: boolean;
    title?: string;
  };
  // events?: Record<string, ActionConfig>;
  children?: ComponentNode[];
  // condition?: Expression;
  // styles?: {
  // className?: string;
  // style?: Record<string, string>;
  // };
}

export type PropValue =
  | string
  | number
  | boolean
  | object
  | BindingValue
  | ExpressionValue;

export interface BindingValue {
  type: "binding";
  source?: string;
  bind?: string;
  target?: "context" | "dataSource" | "runtime";
  path?: string;
  defaultValue?: any;
}

export interface ExpressionValue {
  type: "expression";
  code: string;
  /**
   * Optional dependency hints for lint/analysis only.
   * Does NOT restrict runtime access.
   */
  deps?: ExpressionDeps;
}

export interface ExpressionDeps {
  /**
   * Expected dataSource ids this expression depends on (advisory only).
   */
  dataSources?: string[];
  /**
   * Whether this expression reads from runtime (advisory only).
   */
  runtime?: true;
  /**
   * Optional capability names for documentation/lint (no runtime guarantee).
   */
  capabilities?: string[];
}

export type ActionGraphSchema = {
  id: string;
  context: Record<string, any>;
  actions: Record<string, ActionNode>;
};

export type ActionNode = {
  on: string;
  do: ActionStep[];
};

export type ActionStep =
  | { type: "assign"; to: string; value: string }
  | { type: "callDataSource"; id: string; args?: string[] }
  | { type: "reset"; path: string }
  | { type: "navigate"; path: string }
  | { type: "goBack" };

// type ActionConfig = SingleAction | SingleAction[];

// interface SingleAction {
//   type:
//     | "setState"
//     | "callDataSource"
//     | "navigate"
//     | "showMessage"
//     | "openModal"
//     | "customCode";
//   config: any;
//   condition?: Expression;
// }
