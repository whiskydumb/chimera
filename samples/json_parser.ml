(* a minimal recursive json value type and pretty-printer. *)

type json =
  | Null
  | Bool of bool
  | Number of float
  | String of string
  | Array of json list
  | Object of (string * json) list

let rec to_string = function
  | Null -> "null"
  | Bool b -> if b then "true" else "false"
  | Number n -> Printf.sprintf "%g" n
  | String s -> Printf.sprintf "%S" s
  | Array items -> "[" ^ String.concat "," (List.map to_string items) ^ "]"
  | Object fields ->
      let field (k, v) = Printf.sprintf "%S:%s" k (to_string v) in
      "{" ^ String.concat "," (List.map field fields) ^ "}"
