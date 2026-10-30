using DifferentialEquations, LinearAlgebra, Manifolds

# Динамическая система ограничений
function constraint_dynamics!(du, u, p, t)
Π, J, D, constraints = p
# dx/dt = Π(J - D)Π(x) + Σλᵢ∇cᵢ(x)
du .= Π * (J - D) * Π * u
for (c, λ) in constraints
du .+= λ * gradient(c, u)
end
end

# Поиск аттрактора
function find_attractor(x0, constraints, operators)
prob = ODEProblem(constraint_dynamics!, x0, (0.0, 100.0), (operators..., constraints))
sol = solve(prob, Tsit5(), reltol=1e-8, abstol=1e-8)
    return sol[end]  # конечное состояние = аттрактор
end
