
{} (:package |calcit)
  :configs $ {} (:init-fn |calcot.core/main!) (:reload-fn |calcot.core/reload!)
    :modules $ []
    :version |0.0.1
  :files $ {}
    |calcit.core $ {}
      :ns $ quote (ns calcit.core)
      :defs $ {}
        |main! $ quote
          defn demo () $ echo "\"demo"
