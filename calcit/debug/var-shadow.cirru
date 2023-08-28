
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ [] |./check-args.cirru
  :files $ {}
    |app.main $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ let
                f1 "|local function"
              println check/f1
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns app.main $ :require ([] check-args.main :as check)
