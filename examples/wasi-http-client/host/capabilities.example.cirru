{}
  :component |generated-component/component/component.wasm
  :entry |call-host-http-request
  :arguments $ []
  :arguments-file |/input/request.local.cirru
  :result-file |/output/result.cirru
  :max-response-bytes 65536
  :allowed-origins $ [] |http://127.0.0.1:8123
  :preopens $ []
    {} (:host |input) (:guest |/input) (:access :read)
    {} (:host |output) (:guest |/output) (:access :read-write)
